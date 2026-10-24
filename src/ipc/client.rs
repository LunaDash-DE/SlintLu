//! Connecting to a running shell.

use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::protocol::{Request, Response};

const TIMEOUT: Duration = Duration::from_secs(5);

/// The socket of the shell running in this Wayland session:
/// `$SLINTLU_SOCKET`, or `$XDG_RUNTIME_DIR/slintlu/<WAYLAND_DISPLAY>.sock`.
pub fn socket_path() -> PathBuf {
    if let Some(path) = std::env::var_os("SLINTLU_SOCKET") {
        return path.into();
    }
    let runtime =
        std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let display = std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "wayland-0".into());
    // WAYLAND_DISPLAY may be an absolute path; keep only its name.
    let display =
        Path::new(&display).file_name().map_or(display.clone(), |n| n.to_string_lossy().into());
    runtime.join("slintlu").join(format!("{display}.sock"))
}

/// A connection to the shell's IPC socket.
pub struct Client {
    reader: BufReader<UnixStream>,
}

impl Client {
    pub fn connect(path: &Path) -> io::Result<Self> {
        let stream = UnixStream::connect(path).map_err(|e| {
            io::Error::new(e.kind(), format!("cannot reach slintlu at {}: {e}", path.display()))
        })?;
        stream.set_read_timeout(Some(TIMEOUT))?;
        stream.set_write_timeout(Some(TIMEOUT))?;
        Ok(Self { reader: BufReader::new(stream) })
    }

    /// Sends one request and waits for its response.
    pub fn send(&mut self, request: &Request) -> io::Result<Response> {
        let mut line = serde_json::to_string(request).map_err(io::Error::other)?;
        line.push('\n');
        self.reader.get_mut().write_all(line.as_bytes())?;
        let mut reply = String::new();
        if self.reader.read_line(&mut reply)? == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "slintlu closed the connection",
            ));
        }
        serde_json::from_str(&reply).map_err(io::Error::other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_response_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.sock");
        let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert_eq!(serde_json::from_str::<Request>(&line).unwrap(), Request::Ping);
            writeln!(reader.get_mut(), r#"{{"ok":true,"result":"pong"}}"#).unwrap();
        });
        let mut client = Client::connect(&path).unwrap();
        let response = client.send(&Request::Ping).unwrap();
        assert!(response.ok);
        assert_eq!(response.result, "pong");
        server.join().unwrap();
    }
}
