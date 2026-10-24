//! Batteries from `/sys/class/power_supply`.

use std::fs;
use std::path::Path;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BatteryState {
    Charging,
    Discharging,
    Full,
    NotCharging,
    #[default]
    Unknown,
}

impl BatteryState {
    fn parse(status: &str) -> Self {
        match status {
            "Charging" => Self::Charging,
            "Discharging" => Self::Discharging,
            "Full" => Self::Full,
            "Not charging" => Self::NotCharging,
            _ => Self::Unknown,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Charging => "charging",
            Self::Discharging => "discharging",
            Self::Full => "full",
            Self::NotCharging => "not-charging",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Battery {
    pub name: String,
    /// Charge in `0.0..=1.0`.
    pub level: f32,
    pub state: BatteryState,
    /// Estimated seconds until empty (discharging) or full (charging), when known.
    pub time_remaining: Option<u64>,
}

fn read(dir: &Path, file: &str) -> Option<String> {
    fs::read_to_string(dir.join(file)).ok().map(|s| s.trim().to_owned())
}

fn number(dir: &Path, file: &str) -> Option<f64> {
    read(dir, file)?.parse().ok()
}

fn battery_at(dir: &Path) -> Option<Battery> {
    if read(dir, "type")? != "Battery" || read(dir, "scope").as_deref() == Some("Device") {
        return None;
    }
    let state = BatteryState::parse(&read(dir, "status").unwrap_or_default());
    let level = match (number(dir, "energy_now"), number(dir, "energy_full")) {
        (Some(now), Some(full)) if full > 0.0 => now / full,
        _ => match (number(dir, "charge_now"), number(dir, "charge_full")) {
            (Some(now), Some(full)) if full > 0.0 => now / full,
            _ => number(dir, "capacity")? / 100.0,
        },
    };
    // energy (µWh) / power (µW) or charge (µAh) / current (µA) give hours.
    let rate = number(dir, "power_now").or_else(|| number(dir, "current_now")).unwrap_or(0.0);
    let (now, full) = (
        number(dir, "energy_now").or_else(|| number(dir, "charge_now")),
        number(dir, "energy_full").or_else(|| number(dir, "charge_full")),
    );
    let time_remaining = match (state, now, full) {
        _ if rate <= 0.0 => None,
        (BatteryState::Discharging, Some(now), _) => Some(now / rate * 3600.0),
        (BatteryState::Charging, Some(now), Some(full)) => {
            Some((full - now).max(0.0) / rate * 3600.0)
        }
        _ => None,
    };
    Some(Battery {
        name: dir.file_name()?.to_string_lossy().into_owned(),
        level: level.clamp(0.0, 1.0) as f32,
        state,
        time_remaining: time_remaining.map(|s| s as u64),
    })
}

/// System batteries (peripheral batteries such as mice are excluded).
pub fn batteries() -> Vec<Battery> {
    batteries_in(Path::new("/sys/class/power_supply"))
}

fn batteries_in(root: &Path) -> Vec<Battery> {
    let mut list: Vec<_> = fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| battery_at(&e.path()))
        .collect();
    list.sort_by(|a, b| a.name.cmp(&b.name));
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake(root: &Path, name: &str, files: &[(&str, &str)]) {
        let dir = root.join(name);
        fs::create_dir_all(&dir).unwrap();
        for (file, value) in files {
            fs::write(dir.join(file), value).unwrap();
        }
    }

    #[test]
    fn reads_energy_based_batteries() {
        let root = tempfile::tempdir().unwrap();
        fake(
            root.path(),
            "BAT0",
            &[
                ("type", "Battery\n"),
                ("status", "Discharging\n"),
                ("energy_now", "25000000"),
                ("energy_full", "50000000"),
                ("power_now", "12500000"),
            ],
        );
        fake(root.path(), "AC", &[("type", "Mains")]);
        fake(
            root.path(),
            "hidpp_battery_0",
            &[("type", "Battery"), ("scope", "Device"), ("capacity", "50")],
        );

        let list = batteries_in(root.path());
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "BAT0");
        assert!((list[0].level - 0.5).abs() < 1e-6);
        assert_eq!(list[0].state, BatteryState::Discharging);
        assert_eq!(list[0].time_remaining, Some(7200));
    }

    #[test]
    fn falls_back_to_capacity() {
        let root = tempfile::tempdir().unwrap();
        fake(root.path(), "BAT1", &[("type", "Battery"), ("status", "Full"), ("capacity", "100")]);
        let list = batteries_in(root.path());
        assert_eq!(list[0].level, 1.0);
        assert_eq!(list[0].state, BatteryState::Full);
        assert_eq!(list[0].time_remaining, None);
    }
}
