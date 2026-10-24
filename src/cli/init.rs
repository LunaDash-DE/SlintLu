//! `slintlu init`: write a starter configuration.

use std::path::PathBuf;

use crate::config;

/// The starter shell, embedded from `examples/starter`.
const FILES: &[(&str, &str)] = &[
    ("shell.slint", include_str!("../../examples/starter/shell.slint")),
    ("bar.slint", include_str!("../../examples/starter/bar.slint")),
    ("launcher.slint", include_str!("../../examples/starter/launcher.slint")),
    ("lock.slint", include_str!("../../examples/starter/lock.slint")),
    ("tray.slint", include_str!("../../examples/starter/tray.slint")),
];

pub fn init(dir: Option<PathBuf>, force: bool) -> Result<(), String> {
    let dir = dir.unwrap_or_else(config::default_config_dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for (name, contents) in FILES {
        let path = dir.join(name);
        if path.exists() && !force {
            return Err(format!("{} exists (use --force to overwrite)", path.display()));
        }
        std::fs::write(&path, contents).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("created {}", path.display());
    }
    println!("start it with: slintlu");
    Ok(())
}
