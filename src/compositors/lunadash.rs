//! LunaDash: JSON requests on its control socket (`$LUNADASH_CONTROL`, or
//! `$XDG_RUNTIME_DIR/lunadash-0-control`). LunaDash has no event stream, so the state is
//! polled like its own shell does.

use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

use serde_json::{Value, json};

use super::Backend;
use super::model::{Action, DesktopState, Window, Workspace};

/// LunaDash's fixed number of workspaces.
const WORKSPACES: i64 = 10;
const POLL: Duration = Duration::from_millis(700);
const TIMEOUT: Duration = Duration::from_millis(1500);

pub struct LunaDash {
    socket: PathBuf,
}

fn runtime_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        // SAFETY: getuid has no preconditions and cannot fail.
        .unwrap_or_else(|| PathBuf::from(format!("/run/user/{}", unsafe { libc::getuid() })))
}

impl LunaDash {
    pub fn from_env() -> Option<Self> {
        let explicit = ["LUNADASH_CONTROL", "LUDASH_CONTROL"]
            .iter()
            .find_map(std::env::var_os)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from);
        let socket = explicit.or_else(|| {
            ["lunadash-0-control", "ludash-0-control"]
                .iter()
                .map(|n| runtime_dir().join(n))
                .find(|p| p.exists())
        })?;
        Some(Self { socket })
    }

    /// Sends one request and returns the parsed response.
    pub fn request(&self, method: &str, value: &str) -> io::Result<Value> {
        let mut stream = UnixStream::connect(&self.socket)?;
        stream.set_read_timeout(Some(TIMEOUT))?;
        stream.set_write_timeout(Some(TIMEOUT))?;
        writeln!(stream, "{}", json!({ "method": method, "value": value }))?;
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line)?;
        let response: Value = serde_json::from_str(&line).map_err(io::Error::other)?;
        if let Some(error) = response.get("error").and_then(Value::as_str) {
            return Err(io::Error::other(format!("LunaDash: {error}")));
        }
        Ok(response)
    }
}

/// Builds the shell's view from a `status` response.
fn build_state(status: &Value) -> DesktopState {
    let active = status["workspace"].as_i64().unwrap_or(0);
    let output = status["outputs"]
        .as_array()
        .and_then(|o| o.first())
        .and_then(|o| o["name"].as_str())
        .unwrap_or_default()
        .to_owned();
    let mut state = DesktopState {
        workspaces: (0..WORKSPACES)
            .map(|index| Workspace {
                id: index as u64,
                index: index + 1,
                name: String::new(),
                output: output.clone(),
                active: index == active,
                focused: index == active,
                urgent: false,
                windows: 0,
            })
            .collect(),
        windows: status["clients"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|c| c["mapped"].as_bool().unwrap_or(true))
            .filter_map(|c| {
                Some(Window {
                    id: c["id"].as_u64()?,
                    title: c["title"].as_str().unwrap_or_default().to_owned(),
                    app_id: c["appId"].as_str().unwrap_or_default().to_owned(),
                    workspace: c["workspace"].as_u64(),
                    focused: c["focused"].as_bool().unwrap_or(false),
                    urgent: false,
                })
            })
            .collect(),
        keyboard_layout: String::new(),
    };
    state.normalize();
    state
}

impl Backend for LunaDash {
    fn watch(&self, on_state: &mut dyn FnMut(&DesktopState)) -> io::Result<()> {
        let mut last = None;
        loop {
            let state = build_state(&self.request("status", "")?);
            if last.as_ref() != Some(&state) {
                on_state(&state);
                last = Some(state);
            }
            std::thread::sleep(POLL);
        }
    }

    fn perform(&self, action: &Action) -> io::Result<()> {
        match action {
            Action::FocusWorkspace(id) => self.request("workspace", &id.to_string()),
            Action::FocusWindow(id) => self.request("focus", &id.to_string()),
            Action::CloseWindow(id) => self.request("close", &id.to_string()),
        }
        .map(drop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_state_from_status() {
        let status = json!({
            "workspace": 2,
            "clients": [
                {"id": 7, "title": "Zed", "appId": "dev.zed.Zed", "workspace": 2, "focused": true, "mapped": true},
                {"id": 8, "title": "Term", "appId": "foot", "workspace": 0, "focused": false, "mapped": true},
                {"id": 9, "title": "hidden", "appId": "x", "workspace": 0, "mapped": false}
            ]
        });
        let state = build_state(&status);
        assert_eq!(state.workspaces.len(), 10);
        assert!(state.workspaces[2].active);
        assert_eq!(state.workspaces[2].index, 3);
        assert_eq!(state.workspaces[0].windows, 1);
        assert_eq!(state.windows.len(), 2);
        assert_eq!(state.focused_window().unwrap().title, "Zed");
    }

    #[test]
    fn talks_the_control_protocol() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lunadash-0-control");
        let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let request: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(request, json!({"method": "workspace", "value": "3"}));
            writeln!(reader.get_mut(), "{}", json!({"workspace": 3, "clients": []})).unwrap();
        });
        let backend = LunaDash { socket: path };
        backend.perform(&Action::FocusWorkspace(3)).unwrap();
    }
}
