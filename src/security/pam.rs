//! Password checks through PAM, with a minimal hand-written binding to `libpam`.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::path::Path;

const PAM_SUCCESS: c_int = 0;
const PAM_BUF_ERR: c_int = 5;
const PAM_CONV_ERR: c_int = 19;
const PAM_PROMPT_ECHO_OFF: c_int = 1;
const PAM_PROMPT_ECHO_ON: c_int = 2;

#[repr(C)]
struct PamMessage {
    msg_style: c_int,
    msg: *const c_char,
}

#[repr(C)]
struct PamResponse {
    resp: *mut c_char,
    resp_retcode: c_int,
}

type ConvFn = unsafe extern "C" fn(
    c_int,
    *mut *const PamMessage,
    *mut *mut PamResponse,
    *mut c_void,
) -> c_int;

#[repr(C)]
struct PamConv {
    conv: ConvFn,
    appdata_ptr: *mut c_void,
}

#[repr(C)]
struct PamHandle {
    _private: [u8; 0],
}

#[link(name = "pam")]
unsafe extern "C" {
    fn pam_start(
        service: *const c_char,
        user: *const c_char,
        conv: *const PamConv,
        handle: *mut *mut PamHandle,
    ) -> c_int;
    fn pam_authenticate(handle: *mut PamHandle, flags: c_int) -> c_int;
    fn pam_acct_mgmt(handle: *mut PamHandle, flags: c_int) -> c_int;
    fn pam_end(handle: *mut PamHandle, status: c_int) -> c_int;
    fn pam_strerror(handle: *mut PamHandle, errnum: c_int) -> *const c_char;
}

struct Credentials {
    user: CString,
    password: CString,
}

/// Answers PAM prompts: the password for hidden prompts, the user name for visible ones.
unsafe extern "C" fn conversation(
    count: c_int,
    messages: *mut *const PamMessage,
    responses: *mut *mut PamResponse,
    data: *mut c_void,
) -> c_int {
    if count <= 0 || messages.is_null() || responses.is_null() || data.is_null() {
        return PAM_CONV_ERR;
    }
    let count = count as usize;
    // SAFETY: PAM frees the array and every `resp` with free(), so they come from calloc/strdup.
    let replies = unsafe { libc::calloc(count, size_of::<PamResponse>()) } as *mut PamResponse;
    if replies.is_null() {
        return PAM_BUF_ERR;
    }
    // SAFETY: `data` is the `Credentials` passed to pam_start, alive for the whole call.
    let credentials = unsafe { &*(data as *const Credentials) };
    for i in 0..count {
        // SAFETY: Linux-PAM passes an array of `count` message pointers.
        let message = unsafe { &**messages.add(i) };
        let answer = match message.msg_style {
            PAM_PROMPT_ECHO_OFF => Some(&credentials.password),
            PAM_PROMPT_ECHO_ON => Some(&credentials.user),
            // Informational and error messages need no answer.
            _ => None,
        };
        if let Some(answer) = answer {
            // SAFETY: `answer` is a valid NUL-terminated string; strdup allocates with malloc.
            let copy = unsafe { libc::strdup(answer.as_ptr()) };
            if copy.is_null() {
                // SAFETY: frees what was allocated so far before failing.
                unsafe { free_replies(replies, i) };
                return PAM_BUF_ERR;
            }
            // SAFETY: `i < count`, inside the calloc'ed array.
            unsafe { (*replies.add(i)).resp = copy };
        }
    }
    // SAFETY: `responses` is a valid out pointer provided by PAM.
    unsafe { *responses = replies };
    PAM_SUCCESS
}

/// # Safety
/// `replies` must hold at least `filled` responses allocated by [`conversation`].
unsafe fn free_replies(replies: *mut PamResponse, filled: usize) {
    for i in 0..filled {
        // SAFETY: guaranteed by the caller.
        let resp = unsafe { (*replies.add(i)).resp };
        if !resp.is_null() {
            // SAFETY: overwrite the secret before releasing it.
            unsafe {
                libc::explicit_bzero(resp as *mut c_void, libc::strlen(resp));
                libc::free(resp as *mut c_void);
            }
        }
    }
    // SAFETY: allocated with calloc in `conversation`.
    unsafe { libc::free(replies as *mut c_void) };
}

/// Why authentication failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthError {
    /// Wrong password, locked account or similar; the message comes from PAM.
    Denied(String),
    /// PAM could not be used at all.
    Unavailable(String),
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Denied(m) | Self::Unavailable(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for AuthError {}

/// The PAM service to use: `$SLINTLU_PAM_SERVICE`, `slintlu` when `/etc/pam.d/slintlu`
/// exists, else `login`.
pub fn default_service() -> String {
    if let Ok(service) = std::env::var("SLINTLU_PAM_SERVICE")
        && !service.is_empty()
    {
        return service;
    }
    if Path::new("/etc/pam.d/slintlu").exists() { "slintlu".into() } else { "login".into() }
}

/// The login name of the user running the shell.
pub fn current_user() -> String {
    std::env::var("USER").or_else(|_| std::env::var("LOGNAME")).unwrap_or_default()
}

fn message(handle: *mut PamHandle, code: c_int) -> String {
    // SAFETY: pam_strerror accepts any handle (even null) and returns a static string.
    let text = unsafe { pam_strerror(handle, code) };
    if text.is_null() {
        format!("PAM error {code}")
    } else {
        // SAFETY: non-null, NUL-terminated, owned by PAM.
        unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned()
    }
}

/// Checks `password` for `user` with the PAM `service`. Blocks; call it off the UI thread.
pub fn authenticate(service: &str, user: &str, password: &str) -> Result<(), AuthError> {
    let invalid = |_| AuthError::Unavailable("credentials contain a NUL byte".into());
    let service = CString::new(service).map_err(invalid)?;
    let credentials = Credentials {
        user: CString::new(user).map_err(invalid)?,
        password: CString::new(password).map_err(invalid)?,
    };
    let conv = PamConv {
        conv: conversation,
        appdata_ptr: &credentials as *const Credentials as *mut c_void,
    };
    let mut handle: *mut PamHandle = std::ptr::null_mut();
    // SAFETY: all pointers are valid for the duration of the PAM transaction below.
    let started =
        unsafe { pam_start(service.as_ptr(), credentials.user.as_ptr(), &conv, &mut handle) };
    if started != PAM_SUCCESS || handle.is_null() {
        return Err(AuthError::Unavailable(message(handle, started)));
    }
    // SAFETY: `handle` was returned by a successful pam_start.
    let mut status = unsafe { pam_authenticate(handle, 0) };
    if status == PAM_SUCCESS {
        // SAFETY: as above.
        status = unsafe { pam_acct_mgmt(handle, 0) };
    }
    let result = if status == PAM_SUCCESS {
        Ok(())
    } else {
        Err(AuthError::Denied(message(handle, status)))
    };
    // SAFETY: ends the transaction started above; the handle is not used afterwards.
    unsafe { pam_end(handle, status) };
    let mut password = credentials.password.into_bytes();
    // SAFETY: wipes our copy of the password.
    unsafe { libc::explicit_bzero(password.as_mut_ptr() as *mut c_void, password.len()) };
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    // No test authenticates against a real service: failed attempts count towards
    // pam_faillock and could lock the developer's account.

    #[test]
    fn unknown_services_deny() {
        // Unknown services use /etc/pam.d/other, which denies everything without counting.
        let result = authenticate("slintlu-test-nonexistent-service", "nobody", "x");
        assert!(result.is_err());
    }

    #[test]
    fn nul_bytes_are_refused() {
        assert!(matches!(authenticate("login", "a\0b", "x"), Err(AuthError::Unavailable(_))));
    }
}
