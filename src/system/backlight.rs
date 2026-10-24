//! Screen backlights from `/sys/class/backlight`.

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct Backlight {
    /// Device name, e.g. `intel_backlight`.
    pub name: String,
    pub brightness: u32,
    pub max_brightness: u32,
}

impl Backlight {
    /// Brightness in `0.0..=1.0`.
    pub fn level(&self) -> f32 {
        if self.max_brightness == 0 {
            0.0
        } else {
            self.brightness as f32 / self.max_brightness as f32
        }
    }

    /// The raw value for a level, never fully off (a black screen is hard to undo).
    pub fn raw_for(&self, level: f32) -> u32 {
        let raw = (level.clamp(0.0, 1.0) * self.max_brightness as f32).round() as u32;
        raw.max(1).min(self.max_brightness)
    }
}

fn read_u32(path: &Path) -> Option<u32> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn backlight_at(dir: &Path) -> Option<Backlight> {
    Some(Backlight {
        name: dir.file_name()?.to_string_lossy().into_owned(),
        brightness: read_u32(&dir.join("brightness"))?,
        max_brightness: read_u32(&dir.join("max_brightness")).filter(|m| *m > 0)?,
    })
}

/// The preferred backlight: firmware and platform interfaces before raw ones, as the kernel
/// documentation recommends.
pub fn backlight() -> Option<Backlight> {
    backlight_in(Path::new("/sys/class/backlight"))
}

fn backlight_in(root: &Path) -> Option<Backlight> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(root).ok()?.flatten().map(|e| e.path()).collect();
    let rank = |dir: &PathBuf| match fs::read_to_string(dir.join("type")).unwrap_or_default().trim()
    {
        "firmware" => 0,
        "platform" => 1,
        _ => 2,
    };
    dirs.sort_by_key(|d| (rank(d), d.clone()));
    dirs.iter().find_map(|d| backlight_at(d))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake(root: &Path, name: &str, kind: &str, brightness: u32, max: u32) {
        let dir = root.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("type"), kind).unwrap();
        fs::write(dir.join("brightness"), brightness.to_string()).unwrap();
        fs::write(dir.join("max_brightness"), max.to_string()).unwrap();
    }

    #[test]
    fn prefers_firmware_interfaces() {
        let root = tempfile::tempdir().unwrap();
        fake(root.path(), "acpi_video0", "raw", 5, 10);
        fake(root.path(), "intel_backlight", "firmware", 300, 1200);
        let light = backlight_in(root.path()).unwrap();
        assert_eq!(light.name, "intel_backlight");
        assert_eq!(light.level(), 0.25);
    }

    #[test]
    fn levels_never_turn_the_screen_off() {
        let light = Backlight { name: "x".into(), brightness: 0, max_brightness: 100 };
        assert_eq!(light.raw_for(0.0), 1);
        assert_eq!(light.raw_for(0.5), 50);
        assert_eq!(light.raw_for(2.0), 100);
    }
}
