//! The `Idle` global: idle detection, auto-lock and inhibition, set for the whole shell.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use slint::{Timer, TimerMode};
use slint_interpreter::Value;
use slintlu::idle::{self, IdleWatch};

use crate::scripting::bridge::{bool_arg, number_arg};
use crate::scripting::{ShellHost, host};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);
/// How often a requested inhibition is retried while no shell window is visible yet.
const RETRY: Duration = Duration::from_secs(1);

#[derive(Default)]
struct IdleBinding {
    idle_watch: Option<IdleWatch>,
    lock_watch: Option<IdleWatch>,
    inhibit_wanted: bool,
    inhibit_active: bool,
}

thread_local! {
    static BINDING: RefCell<IdleBinding> = RefCell::default();
}

fn watch_timeout(timeout: Duration) {
    let watch = idle::watch_idle(timeout.max(Duration::from_secs(1)), |idle| {
        host::with(|host| host.set_global("Idle", "idle", Value::Bool(idle)));
    });
    BINDING.with(|b| b.borrow_mut().idle_watch = watch);
}

fn watch_lock_after(timeout: Duration) {
    let watch = (!timeout.is_zero())
        .then(|| {
            idle::watch_idle(timeout.max(Duration::from_secs(1)), |idle| {
                if idle {
                    host::with(|host| super::session::lock(host));
                }
            })
        })
        .flatten();
    BINDING.with(|b| b.borrow_mut().lock_watch = watch);
}

/// Applies the wanted inhibition; retried by a timer until a window is visible.
fn apply_inhibition() {
    BINDING.with(|b| {
        let mut b = b.borrow_mut();
        if b.inhibit_wanted != b.inhibit_active {
            b.inhibit_active = idle::set_idle_inhibited(b.inhibit_wanted);
        }
    });
}

fn millis(args: &[Value]) -> Option<Duration> {
    number_arg(args, 0).filter(|ms| *ms >= 0.0).map(|ms| Duration::from_millis(ms as u64))
}

/// Drops the watches while the platform is still alive.
pub fn shutdown() {
    BINDING.with(|b| *b.borrow_mut() = IdleBinding::default());
}

pub fn install(host: &Rc<ShellHost>) {
    let probe = idle::watch_idle(Duration::from_secs(3600), |_| {});
    host.set_global("Idle", "available", Value::Bool(probe.is_some()));
    drop(probe);
    watch_timeout(DEFAULT_TIMEOUT);

    let weak = Rc::downgrade(host);
    host.on_callback(
        "Idle",
        "set-timeout",
        Rc::new(move |args| {
            if let (Some(timeout), Some(host)) = (millis(args), weak.upgrade()) {
                watch_timeout(timeout);
                host.set_global("Idle", "timeout", Value::Number(timeout.as_millis() as f64));
            }
            Value::Void
        }),
    );
    let weak = Rc::downgrade(host);
    host.on_callback(
        "Idle",
        "set-lock-after",
        Rc::new(move |args| {
            if let (Some(timeout), Some(host)) = (millis(args), weak.upgrade()) {
                watch_lock_after(timeout);
                host.set_global("Idle", "lock-after", Value::Number(timeout.as_millis() as f64));
            }
            Value::Void
        }),
    );
    let weak = Rc::downgrade(host);
    host.on_callback(
        "Idle",
        "set-inhibited",
        Rc::new(move |args| {
            if let (Some(inhibited), Some(host)) = (bool_arg(args, 0), weak.upgrade()) {
                BINDING.with(|b| b.borrow_mut().inhibit_wanted = inhibited);
                apply_inhibition();
                host.set_global("Idle", "inhibited", Value::Bool(inhibited));
            }
            Value::Void
        }),
    );

    let timer = Timer::default();
    timer.start(TimerMode::Repeated, RETRY, apply_inhibition);
    // Lasts as long as the process.
    std::mem::forget(timer);
}
