//! Reading clipboard data from the pipe another client writes into.

use std::io::Read;
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};

/// Give up on clients that never write (or never close) the pipe.
const TIMEOUT: Duration = Duration::from_millis(500);
/// Refuse unreasonably large clipboard contents for a text field.
const LIMIT: usize = 16 * 1024 * 1024;

pub(super) fn read_to_string(mut pipe: impl Read + AsRawFd) -> Option<String> {
    let fd = pipe.as_raw_fd();
    let deadline = Instant::now() + TIMEOUT;
    let mut data = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let remaining = deadline.checked_duration_since(Instant::now())?;
        let mut poll = libc::pollfd { fd, events: libc::POLLIN, revents: 0 };
        // SAFETY: one valid pollfd, a bounded timeout.
        let ready =
            unsafe { libc::poll(&mut poll, 1, remaining.as_millis().min(i32::MAX as u128) as i32) };
        if ready <= 0 {
            return None;
        }
        match pipe.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                data.extend_from_slice(&chunk[..n]);
                if data.len() > LIMIT {
                    return None;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return None,
        }
    }
    Some(String::from_utf8_lossy(&data).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::net::UnixStream;

    #[test]
    fn reads_until_closed() {
        let (reader, mut writer) = UnixStream::pair().unwrap();
        std::thread::spawn(move || {
            writer.write_all("héllo".as_bytes()).unwrap();
        });
        assert_eq!(read_to_string(reader).as_deref(), Some("héllo"));
    }

    #[test]
    fn times_out_on_silent_writers() {
        let (reader, _writer) = UnixStream::pair().unwrap();
        assert_eq!(read_to_string(reader), None);
    }
}
