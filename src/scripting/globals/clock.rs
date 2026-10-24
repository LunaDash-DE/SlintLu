//! The `Clock` global: ticks exactly on every second.

use std::rc::{Rc, Weak};

use slint::{Timer, TimerMode};
use slint_interpreter::Value;
use slintlu::time;

use crate::scripting::ShellHost;
use crate::scripting::bridge::{apply_property, string};

fn format(instance: &slint_interpreter::ComponentInstance, name: &str, fallback: &str) -> String {
    match instance.get_global_property("Clock", name) {
        Ok(Value::String(s)) => s.to_string(),
        _ => fallback.to_owned(),
    }
}

fn tick(host: &ShellHost) {
    let now = time::now();
    // Formats are per window, so every instance formats its own reading.
    host.each_instance(|instance| {
        let reading = time::read(
            now,
            &format(instance, "time-format", "%H:%M"),
            &format(instance, "date-format", "%a %e %b"),
        );
        apply_property(instance, "Clock", "time", string(&reading.time));
        apply_property(instance, "Clock", "date", string(&reading.date));
        apply_property(instance, "Clock", "hour", Value::Number(reading.hour.into()));
        apply_property(instance, "Clock", "minute", Value::Number(reading.minute.into()));
        apply_property(instance, "Clock", "second", Value::Number(reading.second.into()));
    });
}

/// Re-arms a single-shot timer for the next full second, so the clock never drifts.
fn arm(timer: &Rc<Timer>, host: Weak<ShellHost>) {
    let again = Rc::downgrade(timer);
    timer.start(TimerMode::SingleShot, time::until_next_second(time::now()), move || {
        if let Some(host) = host.upgrade() {
            tick(&host);
            if let Some(timer) = again.upgrade() {
                arm(&timer, Rc::downgrade(&host));
            }
        }
    });
}

pub fn install(host: &Rc<ShellHost>) {
    let timer = Rc::new(Timer::default());
    arm(&timer, Rc::downgrade(host));
    // The clock runs as long as the process.
    std::mem::forget(timer);
}

/// Updates new windows immediately instead of waiting for the next second.
pub fn refresh(host: &ShellHost) {
    tick(host);
}
