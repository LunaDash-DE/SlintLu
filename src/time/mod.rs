//! Wall-clock time formatting.

use std::time::Duration;

use chrono::{DateTime, Local, Timelike};

/// A snapshot of the local time, formatted with `strftime`-style patterns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClockReading {
    pub time: String,
    pub date: String,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    /// Seconds since the Unix epoch.
    pub timestamp: i64,
}

/// Formats `now` with the given patterns. Invalid patterns produce an empty string instead of
/// panicking, since they come from user configuration.
pub fn read(now: DateTime<Local>, time_format: &str, date_format: &str) -> ClockReading {
    ClockReading {
        time: format_lossy(&now, time_format),
        date: format_lossy(&now, date_format),
        hour: now.hour(),
        minute: now.minute(),
        second: now.second(),
        timestamp: now.timestamp(),
    }
}

/// The current local time.
pub fn now() -> DateTime<Local> {
    Local::now()
}

fn format_lossy(now: &DateTime<Local>, pattern: &str) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    match write!(out, "{}", now.format(pattern)) {
        Ok(()) => out,
        Err(_) => String::new(),
    }
}

/// Time until the next full second, so clocks tick exactly on the second.
pub fn until_next_second(now: DateTime<Local>) -> Duration {
    let nanos = now.timestamp_subsec_nanos().min(999_999_999);
    Duration::from_nanos(1_000_000_000 - nanos as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(h: u32, m: u32, s: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 10, 24, h, m, s).single().unwrap()
    }

    #[test]
    fn formats_time_and_date() {
        let r = read(at(9, 5, 7), "%H:%M:%S", "%Y-%m-%d");
        assert_eq!(r.time, "09:05:07");
        assert_eq!(r.date, "2026-10-24");
        assert_eq!((r.hour, r.minute, r.second), (9, 5, 7));
    }

    #[test]
    fn invalid_patterns_do_not_panic() {
        let r = read(at(1, 2, 3), "%Q", "%");
        assert_eq!(r.time, "");
    }

    #[test]
    fn next_second_is_within_a_second() {
        let d = until_next_second(Local::now());
        assert!(d > Duration::ZERO && d <= Duration::from_secs(1));
    }
}
