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

/// Everything a shell typically shows about the desktop.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DesktopState {
    /// Sorted by output, then index.
    pub workspaces: Vec<Workspace>,
    pub windows: Vec<Window>,
    pub keyboard_layout: String,
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
