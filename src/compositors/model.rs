//! Compositor-independent description of workspaces and windows.

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Workspace {
    /// Compositor-specific identifier, used in [`Action`]s.
    pub id: u64,
    /// Position as shown to users (1-based where the compositor numbers workspaces).
    pub index: i64,
    pub name: String,
    /// Connector name of the output showing it.
    pub output: String,
    /// Visible on its output.
    pub active: bool,
    /// Visible on the focused output.
    pub focused: bool,
    pub urgent: bool,
    pub windows: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Window {
    pub id: u64,
    pub title: String,
    pub app_id: String,
    pub workspace: Option<u64>,
    pub focused: bool,
    pub urgent: bool,
}

/// The compositor asking the shell to do something, e.g. open its launcher from a keyboard
/// shortcut.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CompositorRequest {
    /// Increases with every request, so repeated requests are distinguishable.
    pub serial: u64,
    /// e.g. "launcher", "settings", "wallpaper-picker", "control-center", "power-menu".
    pub action: String,
    pub argument: String,
}

/// Everything a shell typically shows about the desktop.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DesktopState {
    /// Sorted by output, then index.
    pub workspaces: Vec<Workspace>,
    pub windows: Vec<Window>,
    pub keyboard_layout: String,
    /// The latest request from the compositor, for compositors that send them.
    pub request: Option<CompositorRequest>,
    /// The wallpaper image, for compositors that manage it (a local path; empty otherwise).
    pub wallpaper: String,
    /// A still frame of a video wallpaper (a local path; empty for images).
    pub wallpaper_poster: String,
    /// The compositor plays the (video) wallpaper itself, below the shell's surfaces.
    pub wallpaper_live: bool,
    /// The desktop language chosen in the compositor, e.g. "zh_TW"; empty when it has none.
    pub language: String,
    /// The compositor is ending the session and asks its shell to exit (LunaDash).
    pub session_ending: bool,
}

impl DesktopState {
    pub fn focused_window(&self) -> Option<&Window> {
        self.windows.iter().find(|w| w.focused)
    }

    /// Sorts workspaces and recomputes per-workspace window counts.
    pub fn normalize(&mut self) {
        for ws in &mut self.workspaces {
            ws.windows = self.windows.iter().filter(|w| w.workspace == Some(ws.id)).count() as u32;
        }
        self.workspaces.sort_by(|a, b| a.output.cmp(&b.output).then(a.index.cmp(&b.index)));
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    FocusWorkspace(u64),
    FocusWindow(u64),
    CloseWindow(u64),
    /// Starts the window switcher (or the workspace switcher) without a keyboard shortcut.
    OpenSwitcher {
        workspaces: bool,
    },
    /// Moves the switcher's selection by this many entries.
    StepSwitcher(i32),
    /// Selects a switcher entry by id and switches to it.
    ChooseSwitcher(u64),
    /// Closes the switcher; `accept` switches to the selected entry.
    FinishSwitcher {
        accept: bool,
    },
}

/// One entry of the compositor's window switcher.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SwitcherItem {
    /// The window id, or the workspace id in the workspace switcher.
    pub id: u64,
    pub title: String,
    pub app_id: String,
    pub workspace: u64,
    pub focused: bool,
    pub minimized: bool,
    /// Workspace switcher: the application ids of the windows on the workspace.
    pub apps: Vec<String>,
}

/// The compositor's window switcher (Alt+Tab), for shells that draw it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SwitcherState {
    pub active: bool,
    /// Switching workspaces instead of windows.
    pub workspaces: bool,
    /// The selected entry.
    pub index: usize,
    pub items: Vec<SwitcherItem>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_counts_and_sorts() {
        let mut state = DesktopState {
            workspaces: vec![
                Workspace { id: 2, index: 2, output: "A".into(), ..Default::default() },
                Workspace { id: 1, index: 1, output: "A".into(), ..Default::default() },
            ],
            windows: vec![
                Window { id: 9, workspace: Some(1), focused: true, ..Default::default() },
                Window { id: 8, workspace: Some(1), ..Default::default() },
            ],
            ..Default::default()
        };
        state.normalize();
        assert_eq!(state.workspaces[0].id, 1);
        assert_eq!(state.workspaces[0].windows, 2);
        assert_eq!(state.workspaces[1].windows, 0);
        assert_eq!(state.focused_window().map(|w| w.id), Some(9));
    }
}
