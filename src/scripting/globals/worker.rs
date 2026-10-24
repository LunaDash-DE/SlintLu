//! Background threads that keep a service connection alive.

use std::io;
use std::time::Duration;

const MAX_BACKOFF: Duration = Duration::from_secs(30);

/// Runs `connect` on its own thread forever, reconnecting with exponential back-off whenever it
/// returns (compositor restarts, sound server restarts …).
pub fn spawn_watcher(
    name: &'static str,
    mut connect: impl FnMut() -> io::Result<()> + Send + 'static,
) {
    let spawned = std::thread::Builder::new().name(format!("slintlu-{name}")).spawn(move || {
        let mut backoff = Duration::from_secs(1);
        loop {
            match connect() {
                Ok(()) => backoff = Duration::from_secs(1),
                Err(e) => log::warn!("{name}: {e}; retrying in {}s", backoff.as_secs()),
            }
            std::thread::sleep(backoff);
            backoff = (backoff * 2).min(MAX_BACKOFF);
        }
    });
    if let Err(e) = spawned {
        log::error!("cannot start the {name} thread: {e}");
    }
}

/// Runs a one-off blocking action off the UI thread and logs its failure.
pub fn run_action(name: &'static str, action: impl FnOnce() -> io::Result<()> + Send + 'static) {
    std::thread::spawn(move || {
        if let Err(e) = action() {
            log::warn!("{name}: {e}");
        }
    });
}
