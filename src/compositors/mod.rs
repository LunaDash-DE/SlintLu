//! Knowledge about specific compositors: detection and workspace / window IPC.

mod detect;
mod hyprland;
mod model;
mod niri;
mod sway;

use std::io;
use std::sync::Arc;

pub use detect::Compositor;
pub use model::{Action, DesktopState, Window, Workspace};

/// A compositor's IPC connection.
pub trait Backend: Send + Sync {
    /// Blocks while connected, calling `on_state` with a full snapshot after every change.
    fn watch(&self, on_state: &mut dyn FnMut(&DesktopState)) -> io::Result<()>;
    /// Asks the compositor to do something.
    fn perform(&self, action: &Action) -> io::Result<()>;
}

/// The IPC backend for `compositor`, if it has one and its socket is advertised.
pub fn backend(compositor: Compositor) -> Option<Arc<dyn Backend>> {
    match compositor {
        Compositor::Niri => niri::Niri::from_env().map(|b| Arc::new(b) as Arc<dyn Backend>),
        Compositor::Hyprland => {
            hyprland::Hyprland::from_env().map(|b| Arc::new(b) as Arc<dyn Backend>)
        }
        Compositor::Sway => sway::Sway::from_env().map(|b| Arc::new(b) as Arc<dyn Backend>),
        _ => None,
    }
}
