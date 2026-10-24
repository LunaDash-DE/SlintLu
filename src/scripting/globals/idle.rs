//! The `Idle` global: idle detection, auto-lock and inhibition.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use slint::{Timer, TimerMode};
use slint_interpreter::Value;
use slintlu::idle::{self, IdleWatch};

use crate::scripting::{ShellHost, host};

/// How often the shell's settings are checked for changes.
const POLL: Duration = Duration::from_secs(1);

#[derive(Default)]
struct Settings {
    // Milliseconds, as Slint stores durations.
    timeout_ms: u64,
    lock_after_ms: u64,
    inhibited: bool,
}

#[derive(Default)]
struct IdleBinding {
    applied: Option<(u64, u64, bool)>,
    idle_watch: Option<IdleWatch>,
    lock_watch: Option<IdleWatch>,
}

thread_local! {
    static BINDING: RefCell<IdleBinding> = RefCell::default();
}

fn settings(host: &ShellHost) -> Settings {
    let ms = |name: &str, default: u64| match host.global_property("Idle", name) {
        Some(Value::Number(ms)) if ms >= 0.0 => ms as u64,
        _ => default,
    };
    Settings {
        timeout_ms: ms("timeout", 300_000),
        lock_after_ms: ms("lock-after", 0),
        inhibited: matches!(host.global_property("Idle", "inhibited"), Some(Value::Bool(true))),
    }
}

fn apply(host: &ShellHost) {
    let s = settings(host);
    let key = (s.timeout_ms, s.lock_after_ms, s.inhibited);
    BINDING.with(|b| {
        let mut b = b.borrow_mut();
        let old = b.applied.replace(key);
        if old.map(|k| k.0) != Some(s.timeout_ms) {
            b.idle_watch =
                idle::watch_idle(Duration::from_millis(s.timeout_ms.max(1000)), |idle| {
                    host::with(|host| host.set_global("Idle", "idle", Value::Bool(idle)));
                });
        }
        if old.map(|k| k.1) != Some(s.lock_after_ms) {
            b.lock_watch = (s.lock_after_ms > 0)
                .then(|| {
                    idle::watch_idle(Duration::from_millis(s.lock_after_ms.max(1000)), |idle| {
                        if idle {
                            host::with(|host| super::session::lock(host));
                        }
                    })
                })
                .flatten();
        }
        if old.map(|k| k.2) != Some(s.inhibited) {
            let active = idle::set_idle_inhibited(s.inhibited);
            if s.inhibited && !active {
                // No visible window yet: try again on the next poll.
                b.applied = Some((s.timeout_ms, s.lock_after_ms, false));
            }
        }
    });
}

/// Drops the watches while the platform is still alive.
pub fn shutdown() {
    BINDING.with(|b| *b.borrow_mut() = IdleBinding::default());
}

pub fn install(host: &Rc<ShellHost>) {
    let probe = idle::watch_idle(Duration::from_secs(3600), |_| {});
    host.set_global("Idle", "available", Value::Bool(probe.is_some()));
    drop(probe);

    let weak = Rc::downgrade(host);
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, POLL, move || {
        if let Some(host) = weak.upgrade() {
            apply(&host);
        }
    });
    // Polling lasts as long as the process.
    std::mem::forget(timer);
}
