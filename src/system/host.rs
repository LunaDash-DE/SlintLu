//! Host name, kernel and uptime.

use std::fs;
use std::time::Duration;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HostInfo {
    pub hostname: String,
    pub kernel: String,
    /// `PRETTY_NAME` from os-release, e.g. "Arch Linux".
    pub os: String,
}

fn read_trimmed(path: &str) -> String {
    fs::read_to_string(path).map(|s| s.trim().to_owned()).unwrap_or_default()
}

fn pretty_name(os_release: &str) -> Option<String> {
    os_release
        .lines()
        .find_map(|line| line.strip_prefix("PRETTY_NAME=").map(|v| v.trim_matches('"').to_owned()))
}

pub fn host_info() -> HostInfo {
    let os_release = fs::read_to_string("/etc/os-release")
        .or_else(|_| fs::read_to_string("/usr/lib/os-release"))
        .unwrap_or_default();
    HostInfo {
        hostname: read_trimmed("/proc/sys/kernel/hostname"),
        kernel: read_trimmed("/proc/sys/kernel/osrelease"),
        os: pretty_name(&os_release).unwrap_or_else(|| "Linux".into()),
    }
}

/// Time since boot.
pub fn uptime() -> Duration {
    let seconds = read_trimmed("/proc/uptime")
        .split_whitespace()
        .next()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    Duration::from_secs_f64(seconds.max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_os_release() {
        let release = "NAME=\"Arch Linux\"\nPRETTY_NAME=\"Arch Linux\"\nID=arch\n";
        assert_eq!(pretty_name(release).as_deref(), Some("Arch Linux"));
        assert_eq!(pretty_name("ID=x"), None);
    }

    #[test]
    fn reads_the_running_system() {
        let info = host_info();
        assert!(!info.kernel.is_empty());
        assert!(uptime() > Duration::ZERO);
    }
}
