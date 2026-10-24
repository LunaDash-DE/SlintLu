//! XDG base directories.

use std::env;
use std::path::PathBuf;

fn home() -> PathBuf {
    env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

fn dir(var: &str, fallback: &str) -> PathBuf {
    env::var_os(var)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home().join(fallback))
}

/// `$XDG_DATA_HOME` (`~/.local/share`).
pub fn data_home() -> PathBuf {
    dir("XDG_DATA_HOME", ".local/share")
}

/// `$XDG_CONFIG_HOME` (`~/.config`).
pub fn config_home() -> PathBuf {
    dir("XDG_CONFIG_HOME", ".config")
}

/// `$XDG_STATE_HOME` (`~/.local/state`).
pub fn state_home() -> PathBuf {
    dir("XDG_STATE_HOME", ".local/state")
}

/// `$XDG_DATA_DIRS` (`/usr/local/share:/usr/share`).
pub fn data_dirs() -> Vec<PathBuf> {
    let dirs = env::var("XDG_DATA_DIRS").unwrap_or_default();
    let dirs: Vec<PathBuf> = dirs
        .split(':')
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .collect();
    if dirs.is_empty() {
        vec![PathBuf::from("/usr/local/share"), PathBuf::from("/usr/share")]
    } else {
        dirs
    }
}

/// The user's home directory.
pub fn home_dir() -> PathBuf {
    home()
}
