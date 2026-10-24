//! Hot reload: watches the configuration directory and asks the event loop to reload.

use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use notify::{EventKind, RecursiveMode, Watcher as _};

/// Changes arriving within this window are folded into a single reload.
const DEBOUNCE: Duration = Duration::from_millis(250);

/// Keeps the file watcher alive; reloading stops when dropped.
pub struct Watcher {
    _inner: notify::RecommendedWatcher,
}

impl Watcher {
    /// Calls `on_change` on the Slint event loop whenever a file below `dir` changes.
    pub fn new(dir: &Path, on_change: fn()) -> notify::Result<Self> {
        let (tx, rx) = mpsc::channel::<()>();
        let mut inner =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                let Ok(event) = event else { return };
                let relevant = matches!(
                    event.kind,
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                ) && event.paths.iter().any(|p| !is_editor_temp_file(p));
                if relevant {
                    let _ = tx.send(());
                }
            })?;
        inner.watch(dir, RecursiveMode::Recursive)?;

        std::thread::Builder::new().name("slintlu-watch".into()).spawn(move || {
            while rx.recv().is_ok() {
                // Swallow the burst of events editors produce when saving.
                while rx.recv_timeout(DEBOUNCE).is_ok() {}
                if slint::invoke_from_event_loop(on_change).is_err() {
                    break;
                }
            }
        })?;
        Ok(Self { _inner: inner })
    }
}

fn is_editor_temp_file(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    name.starts_with(".#")
        || name.ends_with('~')
        || name.ends_with(".swp")
        || name.ends_with(".tmp")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_temp_files_are_ignored() {
        assert!(is_editor_temp_file(Path::new("/a/.#shell.slint")));
        assert!(is_editor_temp_file(Path::new("/a/shell.slint~")));
        assert!(is_editor_temp_file(Path::new("/a/.shell.slint.swp")));
        assert!(!is_editor_temp_file(Path::new("/a/shell.slint")));
    }
}
