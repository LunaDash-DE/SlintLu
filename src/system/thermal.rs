//! Temperatures from `/sys/class/hwmon` (preferred) and `/sys/class/thermal`.

use std::fs;
use std::path::Path;

#[derive(Clone, Debug, PartialEq)]
pub struct Temperature {
    /// Sensor name, e.g. "k10temp Tctl" or "x86_pkg_temp".
    pub label: String,
    pub celsius: f32,
}

fn read(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_owned())
}

fn millidegrees(path: &Path) -> Option<f32> {
    read(path)?.parse::<f32>().ok().map(|m| m / 1000.0)
}

/// All readable temperature sensors.
pub fn temperatures() -> Vec<Temperature> {
    let mut temps = Vec::new();
    for hwmon in fs::read_dir("/sys/class/hwmon").into_iter().flatten().flatten() {
        let dir = hwmon.path();
        let chip = read(&dir.join("name")).unwrap_or_else(|| "hwmon".into());
        for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(index) = name.strip_prefix("temp").and_then(|n| n.strip_suffix("_input"))
            else {
                continue;
            };
            let Some(celsius) = millidegrees(&entry.path()) else { continue };
            let label = read(&dir.join(format!("temp{index}_label")))
                .map(|l| format!("{chip} {l}"))
                .unwrap_or_else(|| chip.clone());
            temps.push(Temperature { label, celsius });
        }
    }
    if temps.is_empty() {
        for zone in fs::read_dir("/sys/class/thermal").into_iter().flatten().flatten() {
            let dir = zone.path();
            if let Some(celsius) = millidegrees(&dir.join("temp")) {
                let label = read(&dir.join("type")).unwrap_or_else(|| "thermal".into());
                temps.push(Temperature { label, celsius });
            }
        }
    }
    temps.sort_by(|a, b| a.label.cmp(&b.label));
    temps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readings_are_plausible() {
        for t in temperatures() {
            assert!((-50.0..200.0).contains(&t.celsius), "{t:?}");
        }
    }
}
