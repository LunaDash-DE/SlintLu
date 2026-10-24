//! Memory and swap usage from `/proc/meminfo`.

use std::fs;

/// Sizes in bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MemoryInfo {
    pub total: u64,
    pub available: u64,
    pub swap_total: u64,
    pub swap_free: u64,
}

impl MemoryInfo {
    pub fn used(&self) -> u64 {
        self.total.saturating_sub(self.available)
    }

    /// Used fraction in `0.0..=1.0`.
    pub fn usage(&self) -> f32 {
        if self.total == 0 { 0.0 } else { self.used() as f32 / self.total as f32 }
    }

    pub fn swap_usage(&self) -> f32 {
        if self.swap_total == 0 {
            0.0
        } else {
            self.swap_total.saturating_sub(self.swap_free) as f32 / self.swap_total as f32
        }
    }
}

fn parse(meminfo: &str) -> MemoryInfo {
    let mut info = MemoryInfo::default();
    for line in meminfo.lines() {
        let mut parts = line.split_whitespace();
        let (Some(key), Some(value)) = (parts.next(), parts.next()) else { continue };
        let bytes = value.parse::<u64>().unwrap_or(0) * 1024;
        match key {
            "MemTotal:" => info.total = bytes,
            "MemAvailable:" => info.available = bytes,
            "SwapTotal:" => info.swap_total = bytes,
            "SwapFree:" => info.swap_free = bytes,
            _ => {}
        }
    }
    info
}

pub fn memory() -> MemoryInfo {
    parse(&fs::read_to_string("/proc/meminfo").unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_meminfo() {
        let info = parse(
            "MemTotal:       16000000 kB\nMemFree: 1 kB\nMemAvailable:    4000000 kB\nSwapTotal: 0 kB\n",
        );
        assert_eq!(info.total, 16_000_000 * 1024);
        assert!((info.usage() - 0.75).abs() < 1e-6);
        assert_eq!(info.swap_usage(), 0.0);
    }
}
