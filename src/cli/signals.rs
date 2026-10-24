//! Graceful shutdown on SIGINT / SIGTERM (Ctrl+C, `systemctl --user stop`) and reload on SIGHUP.

use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
use signal_hook::iterator::Signals;

use crate::scripting::host;

/// Starts a thread forwarding signals to the event loop.
pub fn install() {
    let mut signals = match Signals::new([SIGINT, SIGTERM, SIGHUP]) {
        Ok(signals) => signals,
        Err(e) => return log::warn!("cannot handle signals: {e}"),
    };
    let spawned = std::thread::Builder::new().name("slintlu-signals".into()).spawn(move || {
        for signal in signals.forever() {
            if signal == SIGHUP {
                log::info!("SIGHUP: reloading");
                host::post(|host| {
                    host.load();
                });
            } else {
                log::info!("signal {signal}: quitting");
                // Leaves the event loop, so windows, the IPC socket and GPU contexts are
                // released properly.
                let _ = slint::quit_event_loop();
            }
        }
    });
    if let Err(e) = spawned {
        log::warn!("cannot handle signals: {e}");
    }
}
