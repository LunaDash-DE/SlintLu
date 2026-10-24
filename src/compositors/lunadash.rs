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
        request: None,
    };
    state.normalize();
    state
}

/// The serials LunaDash bumps when a shortcut asks the shell for something.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Serials {
    launcher: u64,
    settings: u64,
    picker: u64,
    action: u64,
}

impl Serials {
    fn of(status: &Value) -> Self {
        let n = |key: &str| status[key].as_u64().unwrap_or(0);
        Self {
            launcher: n("launcherSerial"),
            settings: n("settingsSerial"),
            picker: n("pickerSerial"),
            action: n("shellActionSerial"),
        }
    }

    /// The request a change of serials stands for.
    fn request(&self, before: &Self, status: &Value) -> Option<(String, String)> {
        let text = |key: &str| status[key].as_str().unwrap_or_default().to_owned();
        if self.launcher != before.launcher {
            let open = status["launcherOpen"].as_bool().unwrap_or(true);
            return Some(("launcher".into(), open.to_string()));
        }
        if self.settings != before.settings {
            return Some(("settings".into(), text("settingsPage")));
        }
        if self.picker != before.picker {
            return Some(("wallpaper-picker".into(), String::new()));
        }
        if self.action != before.action {
            let action = match text("shellAction").as_str() {
                "launchOrbit" => "orbit-launcher",
                "openControlCenter" => "control-center",
                "openClipboard" => "clipboard",
                "openPowerMenu" => "power-menu",
                other => return Some((other.to_owned(), String::new())),
            };
            return Some((action.into(), String::new()));
        }
        None
    }
}

impl Backend for LunaDash {
    fn watch(&self, on_state: &mut dyn FnMut(&DesktopState)) -> io::Result<()> {
        let mut last: Option<DesktopState> = None;
        let mut serials: Option<Serials> = None;
        let mut request: Option<crate::compositors::CompositorRequest> = None;
        loop {
            let status = self.request("status", "")?;
            let mut state = build_state(&status);
            let now = Serials::of(&status);
            // The first snapshot only records the serials: old requests are not replayed.
            if let Some(before) = &serials
                && let Some((action, argument)) = now.request(before, &status)
            {
                let serial = request.as_ref().map_or(1, |r| r.serial + 1);
                request = Some(crate::compositors::CompositorRequest { serial, action, argument });
            }
            serials = Some(now);
            state.request = request.clone();
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

    fn command(&self, method: &str, value: &str) -> io::Result<String> {
        self.request(method, value).map(|v| v.to_string())
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
    fn serial_changes_become_requests() {
        let before = Serials::of(&json!({"launcherSerial": 1, "shellActionSerial": 4}));
        let status = json!({"launcherSerial": 1, "shellActionSerial": 5, "shellAction": "openControlCenter"});
        let now = Serials::of(&status);
        assert_eq!(now.request(&before, &status), Some(("control-center".into(), String::new())));
        let status = json!({"launcherSerial": 2, "launcherOpen": false, "shellActionSerial": 5});
        assert_eq!(
            Serials::of(&status).request(&now, &status),
            Some(("launcher".into(), "false".into()))
        );
        assert_eq!(now.request(&now, &status), None);
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
