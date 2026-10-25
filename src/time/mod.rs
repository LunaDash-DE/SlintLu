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

/// One cell of a month calendar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CalendarDay {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    /// Belongs to the requested month (not a padding day of a neighbour).
    pub in_month: bool,
    pub today: bool,
    pub weekend: bool,
}

/// Six weeks of days covering `month` of `year`, weeks starting on Monday (`monday_first`) or
/// Sunday.
pub fn month_grid(
    year: i32,
    month: u32,
    monday_first: bool,
    today: chrono::NaiveDate,
) -> Vec<CalendarDay> {
    use chrono::{Datelike, NaiveDate, Weekday};
    let Some(first) = NaiveDate::from_ymd_opt(year, month.clamp(1, 12), 1) else {
        return Vec::new();
    };
    let offset = if monday_first {
        first.weekday().num_days_from_monday()
    } else {
        first.weekday().num_days_from_sunday()
    };
    let start = first - chrono::Duration::days(offset.into());
    (0..42)
        .map(|i| {
            let date = start + chrono::Duration::days(i);
            CalendarDay {
                year: date.year(),
                month: date.month(),
                day: date.day(),
                in_month: date.month() == first.month(),
                today: date == today,
                weekend: matches!(date.weekday(), Weekday::Sat | Weekday::Sun),
            }
        })
        .collect()
}

/// Formats a calendar date with a `strftime` pattern; empty for invalid input.
pub fn format_date(year: i32, month: u32, day: u32, pattern: &str) -> String {
    format_date_in(year, month, day, pattern, crate::i18n::current_language().as_deref())
}

/// [`format_date`] with weekday and month names in `language` ("zh_TW", "de", …); English
/// for an unknown or missing language.
pub fn format_date_in(
    year: i32,
    month: u32,
    day: u32,
    pattern: &str,
    language: Option<&str>,
) -> String {
    use std::fmt::Write;
    let Some(date) = chrono::NaiveDate::from_ymd_opt(year, month, day) else {
        return String::new();
    };
    let mut out = String::new();
    let written = match language.and_then(date_locale) {
        Some(locale) => write!(out, "{}", date.format_localized(pattern, locale)),
        None => write!(out, "{}", date.format(pattern)),
    };
    match written {
        Ok(()) => out,
        Err(_) => String::new(),
    }
}

/// The chrono locale for a language tag: "zh_TW" as is, a bare "de" as "de_DE".
fn date_locale(language: &str) -> Option<chrono::Locale> {
    let tag = language.replace('-', "_");
    if let Ok(locale) = chrono::Locale::try_from(tag.as_str()) {
        return Some(locale);
    }
    let base = tag.split('_').next().unwrap_or_default();
    chrono::Locale::try_from(format!("{base}_{}", base.to_uppercase()).as_str()).ok()
}

#[cfg(test)]
mod calendar_tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn dates_use_the_interface_language() {
        assert_eq!(format_date_in(2026, 10, 25, "%A %B", None), "Sunday October");
        assert_eq!(format_date_in(2026, 10, 25, "%A", Some("zh_TW")), "週日");
        assert_eq!(format_date_in(2026, 10, 25, "%A", Some("de")), "Sonntag");
        assert_eq!(format_date_in(2026, 10, 25, "%A", Some("xx")), "Sunday");
        assert_eq!(format_date_in(2026, 10, 25, "%Y年%-m月%-d日", Some("zh_TW")), "2026年10月25日");
    }

    #[test]
    fn october_2026_starts_on_thursday() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 24).unwrap();
        let grid = month_grid(2026, 10, true, today);
        assert_eq!(grid.len(), 42);
        // Monday 28 September is the first cell; 1 October is the fourth.
        assert_eq!((grid[0].month, grid[0].day, grid[0].in_month), (9, 28, false));
        assert_eq!((grid[3].month, grid[3].day, grid[3].in_month), (10, 1, true));
        assert!(grid.iter().any(|d| d.today && d.day == 24));
        assert!(grid[5].weekend && grid[6].weekend);
        let sunday_first = month_grid(2026, 10, false, today);
        assert_eq!((sunday_first[0].month, sunday_first[0].day), (9, 27));
    }

    #[test]
    fn formats_dates() {
        assert_eq!(format_date_in(2026, 10, 24, "%B %Y", None), "October 2026");
        assert_eq!(format_date_in(2026, 2, 30, "%d", None), "");
    }
}
