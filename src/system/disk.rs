//! Filesystem usage through `statvfs`.

use std::path::Path;

/// Sizes in bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DiskUsage {
    pub total: u64,
    pub available: u64,
}

impl DiskUsage {
    pub fn usage(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            self.total.saturating_sub(self.available) as f32 / self.total as f32
        }
    }
}

/// Usage of the filesystem containing `path`.
pub fn disk_usage(path: &Path) -> Option<DiskUsage> {
    let stat = rustix::fs::statvfs(path).ok()?;
    Some(DiskUsage {
        total: stat.f_blocks * stat.f_frsize,
        available: stat.f_bavail * stat.f_frsize,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_has_a_size() {
        let usage = disk_usage(Path::new("/")).unwrap();
        assert!(usage.total > 0);
        assert!((0.0..=1.0).contains(&usage.usage()));
    }
}
