//! Exercises the polkit agent without entering a password: registers an agent for one
//! process, triggers a request with `pkcheck` and cancels it immediately.
//!
//! `cargo run --example polkit_cancel`

use std::process::Command;
use std::sync::{Arc, OnceLock};

use slintlu::security::polkit::{self, PolkitEvent, PolkitHandle, Subject};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut subject = Command::new("sleep").arg("30").spawn()?;
    let handle: Arc<OnceLock<PolkitHandle>> = Arc::default();
    let sink_handle = handle.clone();
    let registered = polkit::register(Subject::Process(subject.id()), move |event| {
        println!("agent event: {event:?}");
        if let (PolkitEvent::Begin { cookie, .. }, Some(handle)) = (&event, sink_handle.get()) {
            handle.cancel(cookie);
        }
    })?;
    let _ = handle.set(registered);
    println!("registered for pid {}", subject.id());

    let status = Command::new("pkcheck")
        .args([
            "--action-id",
            "org.freedesktop.policykit.exec",
            "--allow-user-interaction",
            "--process",
        ])
        .arg(subject.id().to_string())
        .status()?;
    println!(
        "pkcheck exit status: {status} (non-zero: not authorized, as expected after cancelling)"
    );
    handle.get().map(PolkitHandle::unregister);
    subject.kill()?;
    Ok(())
}
