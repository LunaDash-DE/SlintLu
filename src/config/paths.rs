//! XDG base directories and config file resolution.

use std::path::{Path, PathBuf};

use slintlu::util::xdg;

/// `$XDG_CONFIG_HOME/slintlu`.
pub fn default_config_dir() -> PathBuf {
    xdg::config_home().join("slintlu")
}

/// `$XDG_DATA_HOME/slintlu`.
pub fn data_home() -> PathBuf {
    xdg::data_home().join("slintlu")
}

/// Turns the `--config` argument (a file or a directory, or nothing) into the shell's entry
/// file. Directories are expected to contain a `shell.slint`.
pub fn resolve_config(explicit: Option<&Path>) -> Result<PathBuf, String> {
    let candidate = match explicit {
        Some(path) if path.is_dir() => path.join("shell.slint"),
        Some(path) => path.to_path_buf(),
        None => default_config_dir().join("shell.slint"),
    };
    if candidate.is_file() {
        candidate.canonicalize().map_err(|e| format!("{}: {e}", candidate.display()))
    } else if explicit.is_none() {
        Err(format!(
            "no shell configuration at {}\nhint: run `slintlu init` to create one, or pass --config",
            candidate.display()
        ))
    } else {
        Err(format!("{} does not exist", candidate.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directories_resolve_to_shell_slint() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("shell.slint"), "").unwrap();
        let resolved = resolve_config(Some(dir.path())).unwrap();
        assert!(resolved.ends_with("shell.slint"));
        assert!(resolved.is_absolute());
    }

    #[test]
    fn missing_files_are_reported() {
        let dir = tempfile::tempdir().unwrap();
        let err = resolve_config(Some(&dir.path().join("nope.slint"))).unwrap_err();
        assert!(err.contains("does not exist"));
    }
}
