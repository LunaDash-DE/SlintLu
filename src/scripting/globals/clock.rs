//! The `Clock` global: ticks exactly on every second.

use std::rc::{Rc, Weak};

use slint::{Timer, TimerMode};
use slint_interpreter::Value;
use slintlu::time;

use crate::scripting::ShellHost;
use chrono::Datelike;

use crate::scripting::bridge::{
    apply_property, bool_arg, list_value, number_arg, string, string_arg, struct_value,
};

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
        apply_property(instance, "Clock", "year", Value::Number(now.year().into()));
        apply_property(instance, "Clock", "month", Value::Number(now.month().into()));
        apply_property(instance, "Clock", "day", Value::Number(now.day().into()));
        apply_property(
            instance,
            "Clock",
            "weekday",
            Value::Number(now.weekday().number_from_monday().into()),
        );
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
    host.on_callback(
        "Clock",
        "month-grid",
        Rc::new(|args| {
            let year = number_arg(args, 0).unwrap_or(1970.0) as i32;
            let month = number_arg(args, 1).unwrap_or(1.0) as u32;
            let monday_first = bool_arg(args, 2).unwrap_or(true);
            let today = time::now().date_naive();
            list_value(time::month_grid(year, month, monday_first, today).into_iter().map(|d| {
                struct_value([
                    ("year", Value::Number(d.year.into())),
                    ("month", Value::Number(d.month.into())),
                    ("day", Value::Number(d.day.into())),
                    ("in-month", Value::Bool(d.in_month)),
                    ("today", Value::Bool(d.today)),
                    ("weekend", Value::Bool(d.weekend)),
                ])
            }))
        }),
    );
    host.on_callback(
        "Clock",
        "format-date",
        Rc::new(|args| {
            let n = |i| number_arg(args, i).unwrap_or(0.0);
            string(time::format_date(
                n(0) as i32,
                n(1) as u32,
                n(2) as u32,
                &string_arg(args, 3).unwrap_or_default(),
            ))
        }),
    );
    let timer = Rc::new(Timer::default());
    arm(&timer, Rc::downgrade(host));
    // The clock runs as long as the process.
    std::mem::forget(timer);
}

/// Updates new windows immediately instead of waiting for the next second.
pub fn refresh(host: &ShellHost) {
    tick(host);
}
