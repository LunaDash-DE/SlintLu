//! A polkit authentication agent: the password dialog applications get when they need
//! administrator rights (`pkexec`, mounting disks, changing system settings …).
//!
//! Passwords are checked by polkit's own helper, reached through its systemd socket
//! (`/run/polkit/agent-helper.socket`, polkit ≥ 125) or the classic setuid
//! `polkit-agent-helper-1`; SlintLu never needs privileges itself.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use futures_channel::oneshot;
use zbus::{Connection, OwnedVariant, Variant, interface, proxy};

const AGENT_PATH: &str = "/org/slintlu/PolicyKit1/AuthenticationAgent";
const HELPER_SOCKET: &str = "/run/polkit/agent-helper.socket";
const HELPERS: [&str; 3] = [
    "/usr/lib/polkit-1/polkit-agent-helper-1",
    "/usr/libexec/polkit-agent-helper-1",
    "/usr/lib/policykit-1/polkit-agent-helper-1",
];
/// Wrong passwords allowed before the request fails.
const ATTEMPTS: u32 = 3;

#[proxy(
    interface = "org.freedesktop.PolicyKit1.Authority",
    default_service = "org.freedesktop.PolicyKit1",
    default_path = "/org/freedesktop/PolicyKit1/Authority"
)]
trait Authority {
    fn register_authentication_agent(
        &self,
        subject: &(&str, HashMap<&str, Variant<'_>>),
        locale: &str,
        object_path: &str,
    ) -> zbus::Result<()>;
    fn unregister_authentication_agent(
        &self,
        subject: &(&str, HashMap<&str, Variant<'_>>),
        object_path: &str,
    ) -> zbus::Result<()>;
}

/// Who the agent answers for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Subject {
    /// A login session (`$XDG_SESSION_ID`): the normal case for a desktop shell.
    Session(String),
    /// A single process and its children (like `pkttyagent --process`).
    Process(u32),
}

impl Subject {
    /// The session SlintLu runs in.
    pub fn current_session() -> Option<Self> {
        std::env::var("XDG_SESSION_ID").ok().filter(|s| !s.is_empty()).map(Self::Session)
    }

    fn start_time(pid: u32) -> u64 {
        // Field 22 of /proc/<pid>/stat, after the parenthesised command name.
        std::fs::read_to_string(format!("/proc/{pid}/stat"))
            .ok()
            .and_then(|s| s.rsplit_once(')').map(|(_, rest)| rest.to_owned()))
            .and_then(|rest| rest.split_whitespace().nth(19).and_then(|v| v.parse().ok()))
            .unwrap_or(0)
    }

    fn to_dbus(&self) -> (&'static str, HashMap<&'static str, Variant<'static>>) {
        match self {
            Self::Session(id) => {
                ("unix-session", HashMap::from([("session-id", Variant::from(id.clone()))]))
            }
            Self::Process(pid) => (
                "unix-process",
                HashMap::from([
                    ("pid", Variant::from(*pid)),
                    ("start-time", Variant::from(Self::start_time(*pid))),
                ]),
            ),
        }
    }
}

/// What the shell's dialog shows and is asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PolkitEvent {
    /// A request starts.
    Begin { cookie: String, action_id: String, message: String, icon_name: String, user: String },
    /// The helper asks for input (normally the password; `echo` for visible input).
    Prompt { cookie: String, prompt: String, echo: bool },
    /// A message from PAM.
    Info { cookie: String, text: String, error: bool },
    /// The request ended.
    Finished { cookie: String, success: bool },
}

type Sink = Arc<dyn Fn(PolkitEvent) + Send + Sync>;
/// `Some(text)` answers a prompt, `None` cancels.
type Responders = Arc<Mutex<HashMap<String, mpsc::Sender<Option<String>>>>>;

/// Lets the shell answer prompts.
#[derive(Clone)]
pub struct PolkitHandle {
    responders: Responders,
    connection: Connection,
    subject: Subject,
}

impl PolkitHandle {
    /// Answers the current prompt of a request.
    pub fn respond(&self, cookie: &str, text: String) {
        if let Some(tx) = self.responders.lock().unwrap_or_else(|e| e.into_inner()).get(cookie) {
            let _ = tx.send(Some(text));
        }
    }

    /// Cancels a request.
    pub fn cancel(&self, cookie: &str) {
        if let Some(tx) = self.responders.lock().unwrap_or_else(|e| e.into_inner()).get(cookie) {
            let _ = tx.send(None);
        }
    }

    /// Stops being the agent.
    pub fn unregister(&self) {
        let subject = self.subject.to_dbus();
        let _ = zbus::block_on(async {
            AuthorityProxy::new(&self.connection)
                .await?
                .unregister_authentication_agent(&subject, AGENT_PATH)
                .await
        });
    }
}

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.freedesktop.PolicyKit1.Error")]
enum PolkitError {
    #[zbus(error)]
    ZBus(zbus::Error),
    Failed(String),
    Cancelled(String),
}

struct Agent {
    sink: Sink,
    responders: Responders,
}

/// The login name for a uid.
fn user_name(uid: u32) -> Option<String> {
    let mut buffer = vec![0 as libc::c_char; 4096];
    // SAFETY: zeroed passwd struct and a buffer for getpwuid_r to fill.
    let mut passwd: libc::passwd = unsafe { std::mem::zeroed() };
    let mut result = std::ptr::null_mut();
    // SAFETY: all pointers are valid for the call; the strings live in `buffer`.
    let rc = unsafe {
        libc::getpwuid_r(uid, &mut passwd, buffer.as_mut_ptr(), buffer.len(), &mut result)
    };
    if rc != 0 || result.is_null() {
        return None;
    }
    // SAFETY: getpwuid_r succeeded, so pw_name points to a NUL-terminated string in `buffer`.
    Some(unsafe { std::ffi::CStr::from_ptr(passwd.pw_name) }.to_string_lossy().into_owned())
}

/// Picks whose password to ask for: the current user when allowed, else the first user listed.
fn pick_identity(identities: &[(String, HashMap<String, OwnedVariant>)]) -> Option<String> {
    let me = super::current_user();
    let users: Vec<String> = identities
        .iter()
        .filter(|(kind, _)| kind == "unix-user")
        .filter_map(|(_, details)| details.get("uid").and_then(|v| u32::try_from(&**v).ok()))
        .filter_map(user_name)
        .collect();
    users.iter().find(|u| **u == me).cloned().or_else(|| users.into_iter().next())
}

/// Starts polkit's helper for `user`, returning a stream to talk to it.
fn start_helper(
    user: &str,
    cookie: &str,
) -> std::io::Result<(Box<dyn BufRead + Send>, Box<dyn Write + Send>)> {
    if Path::new(HELPER_SOCKET).exists() {
        match UnixStream::connect(HELPER_SOCKET) {
            Ok(stream) => {
                let mut writer = stream.try_clone()?;
                // Socket-activated helpers read the user name before the cookie.
                writer.write_all(format!("{user}\n{cookie}\n").as_bytes())?;
                return Ok((Box::new(BufReader::new(stream)), Box::new(writer)));
            }
            Err(e) => log::debug!("polkit: {HELPER_SOCKET}: {e}, trying the setuid helper"),
        }
    }
    let helper = HELPERS.iter().find(|p| Path::new(p).exists()).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "polkit-agent-helper-1 not found")
    })?;
    let mut child =
        Command::new(helper).arg(user).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn()?;
    let mut stdin = child.stdin.take().ok_or_else(|| std::io::Error::other("no helper stdin"))?;
    let stdout = child.stdout.take().ok_or_else(|| std::io::Error::other("no helper stdout"))?;
    stdin.write_all(format!("{cookie}\n").as_bytes())?;
    std::thread::spawn(move || child.wait());
    Ok((Box::new(BufReader::new(stdout)), Box::new(stdin)))
}

/// Decodes the C-style escapes the helper uses (`\n`, `\t`, `\\`, octal).
fn unescape(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some(d @ '0'..='7') => {
                let mut value = d.to_digit(8).unwrap_or(0);
                for _ in 0..2 {
                    match chars.peek().and_then(|c| c.to_digit(8)) {
                        Some(v) => {
                            value = value * 8 + v;
                            chars.next();
                        }
                        None => break,
                    }
                }
                out.push(char::from_u32(value).unwrap_or('?'));
            }
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

enum Outcome {
    Success,
    Failure,
    Cancelled,
}

/// One conversation with the helper.
fn converse(
    sink: &Sink,
    rx: &mpsc::Receiver<Option<String>>,
    user: &str,
    cookie: &str,
) -> std::io::Result<Outcome> {
    let (mut reader, mut writer) = start_helper(user, cookie)?;
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            return Ok(Outcome::Failure);
        }
        let text = unescape(line.trim_end_matches('\n'));
        let cookie = cookie.to_owned();
        if let Some(prompt) = text.strip_prefix("PAM_PROMPT_ECHO_OFF ") {
            sink(PolkitEvent::Prompt { cookie, prompt: prompt.to_owned(), echo: false });
        } else if let Some(prompt) = text.strip_prefix("PAM_PROMPT_ECHO_ON ") {
            sink(PolkitEvent::Prompt { cookie, prompt: prompt.to_owned(), echo: true });
        } else if let Some(message) = text.strip_prefix("PAM_ERROR_MSG ") {
            sink(PolkitEvent::Info { cookie, text: message.to_owned(), error: true });
            continue;
        } else if let Some(message) = text.strip_prefix("PAM_TEXT_INFO ") {
            sink(PolkitEvent::Info { cookie, text: message.to_owned(), error: false });
            continue;
        } else if text.starts_with("SUCCESS") {
            return Ok(Outcome::Success);
        } else if text.starts_with("FAILURE") {
            return Ok(Outcome::Failure);
        } else {
            log::warn!("polkit: unexpected helper output {text:?}");
            return Ok(Outcome::Failure);
        }
        match rx.recv() {
            Ok(Some(mut answer)) => {
                answer.push('\n');
                let written = writer.write_all(answer.as_bytes());
                // SAFETY: wipes our copy of the secret.
                unsafe {
                    libc::explicit_bzero(answer.as_mut_ptr() as *mut libc::c_void, answer.len())
                };
                written?;
            }
            // Cancelled: closing the helper's input ends its PAM conversation.
            Ok(None) | Err(_) => return Ok(Outcome::Cancelled),
        }
    }
}

#[interface(name = "org.freedesktop.PolicyKit1.AuthenticationAgent")]
impl Agent {
    async fn begin_authentication(
        &self,
        action_id: String,
        message: String,
        icon_name: String,
        _details: HashMap<String, String>,
        cookie: String,
        identities: Vec<(String, HashMap<String, OwnedVariant>)>,
    ) -> Result<(), PolkitError> {
        let user = pick_identity(&identities)
            .ok_or_else(|| PolkitError::Failed("no user to authenticate as".into()))?;
        let (tx, rx) = mpsc::channel();
        self.responders.lock().unwrap_or_else(|e| e.into_inner()).insert(cookie.clone(), tx);
        (self.sink)(PolkitEvent::Begin {
            cookie: cookie.clone(),
            action_id,
            message,
            icon_name,
            user: user.clone(),
        });

        let (done, finished) = oneshot::channel();
        let sink = self.sink.clone();
        let session_cookie = cookie.clone();
        std::thread::spawn(move || {
            let mut outcome = Outcome::Failure;
            for attempt in 1..=ATTEMPTS {
                // A cancel that arrived before the helper was started: never begin a PAM
                // conversation just to abort it (pam_faillock may count that as a failure).
                if let Ok(None) = rx.try_recv() {
                    outcome = Outcome::Cancelled;
                    break;
                }
                outcome = match converse(&sink, &rx, &user, &session_cookie) {
                    Ok(outcome) => outcome,
                    Err(e) => {
                        log::error!("polkit: cannot talk to the helper: {e}");
                        Outcome::Failure
                    }
                };
                match outcome {
                    Outcome::Failure if attempt < ATTEMPTS => sink(PolkitEvent::Info {
                        cookie: session_cookie.clone(),
                        text: "Authentication failed, please try again.".into(),
                        error: true,
                    }),
                    _ => break,
                }
            }
            let _ = done.send(outcome);
        });

        let outcome = finished.await.unwrap_or(Outcome::Cancelled);
        self.responders.lock().unwrap_or_else(|e| e.into_inner()).remove(&cookie);
        let success = matches!(outcome, Outcome::Success);
        (self.sink)(PolkitEvent::Finished { cookie, success });
        match outcome {
            Outcome::Success => Ok(()),
            Outcome::Failure => Err(PolkitError::Failed("authentication failed".into())),
            Outcome::Cancelled => Err(PolkitError::Cancelled("cancelled by the user".into())),
        }
    }

    async fn cancel_authentication(&self, cookie: String) {
        if let Some(tx) = self.responders.lock().unwrap_or_else(|e| e.into_inner()).get(&cookie) {
            let _ = tx.send(None);
        }
    }
}

/// Registers as the polkit agent for `subject`. Fails when another agent (another shell,
/// polkit-gnome, …) is already registered for it.
pub fn register(
    subject: Subject,
    sink: impl Fn(PolkitEvent) + Send + Sync + 'static,
) -> zbus::Result<PolkitHandle> {
    let responders: Responders = Arc::default();
    let connection = zbus::block_on(async {
        let connection = Connection::system().await?;
        connection
            .object_server()
            .at(AGENT_PATH, Agent { sink: Arc::new(sink), responders: responders.clone() })
            .await?;
        let locale = std::env::var("LANG").unwrap_or_else(|_| "C".into());
        AuthorityProxy::new(&connection)
            .await?
            .register_authentication_agent(&subject.to_dbus(), &locale, AGENT_PATH)
            .await?;
        Ok::<_, zbus::Error>(connection)
    })?;
    Ok(PolkitHandle { responders, connection, subject })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unescapes_helper_lines() {
        assert_eq!(unescape(r"Password: "), "Password: ");
        assert_eq!(unescape(r"line\nnext\ttab\\"), "line\nnext\ttab\\");
        assert_eq!(unescape(r"x\101\102"), "xAB");
    }

    #[test]
    fn process_subjects_have_a_start_time() {
        let (kind, details) = Subject::Process(std::process::id()).to_dbus();
        assert_eq!(kind, "unix-process");
        assert!(matches!(details.get("start-time"), Some(Variant::U64(t)) if *t > 0));
    }

    #[test]
    fn prefers_the_current_user() {
        // SAFETY: getuid has no preconditions and cannot fail.
        let my_uid = unsafe { libc::getuid() };
        let me = user_name(my_uid).unwrap();
        let identity = |uid: u32| {
            ("unix-user".to_string(), HashMap::from([("uid".to_string(), OwnedVariant::from(uid))]))
        };
        assert_eq!(pick_identity(&[identity(0), identity(my_uid)]).as_deref(), Some(me.as_str()));
        assert_eq!(pick_identity(&[identity(0)]).as_deref(), Some("root"));
        assert_eq!(pick_identity(&[]), None);
    }
}
