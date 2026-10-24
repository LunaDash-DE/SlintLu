//! Human readable sizes, rates and durations.

use std::time::Duration;

/// `1536` → `"1.5 KiB"`.
pub fn bytes(value: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    let mut size = value as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 { format!("{value} B") } else { format!("{size:.1} {}", UNITS[unit]) }
}

/// Bytes per second in decimal units, as network tools show them: `1_200_000.0` → `"1.2 MB/s"`.
pub fn rate(bytes_per_second: f64) -> String {
    const UNITS: [&str; 5] = ["B/s", "kB/s", "MB/s", "GB/s", "TB/s"];
    let mut value = bytes_per_second.max(0.0);
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{value:.0} {}", UNITS[0])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// `Duration::from_secs(11_520)` → `"3h 12m"`; days are shown for long durations.
pub fn duration(value: Duration) -> String {
    let minutes = value.as_secs() / 60;
    let (days, hours, minutes) = (minutes / 1440, minutes / 60 % 24, minutes % 60);
    match (days, hours) {
        (0, 0) => format!("{minutes}m"),
        (0, _) => format!("{hours}h {minutes}m"),
        _ => format!("{days}d {hours}h"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_bytes() {
        assert_eq!(bytes(512), "512 B");
        assert_eq!(bytes(1536), "1.5 KiB");
        assert_eq!(bytes(5 * 1024 * 1024 * 1024), "5.0 GiB");
    }

    #[test]
    fn formats_rates() {
        assert_eq!(rate(999.0), "999 B/s");
        assert_eq!(rate(1_200_000.0), "1.2 MB/s");
        assert_eq!(rate(-5.0), "0 B/s");
    }

    #[test]
    fn formats_durations() {
        assert_eq!(duration(Duration::from_secs(59)), "0m");
        assert_eq!(duration(Duration::from_secs(11_520)), "3h 12m");
        assert_eq!(duration(Duration::from_secs(3 * 86_400 + 7200)), "3d 2h");
    }
}
