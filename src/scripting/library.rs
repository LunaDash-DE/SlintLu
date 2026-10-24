//! The `@slintlu` Slint library, embedded in the binary and extracted to disk so that both the
//! compiler and editors (slint-lsp) can find it.

use std::io;
use std::path::PathBuf;

/// Library sources, relative to the library directory.
const FILES: &[(&str, &str)] = &[
    ("slintlu.slint", include_str!("../ui/slintlu.slint")),
    ("types.slint", include_str!("../ui/types.slint")),
    ("runtime.slint", include_str!("../ui/runtime.slint")),
    ("windows.slint", include_str!("../ui/windows.slint")),
];

/// The library name used in imports: `import { PanelWindow } from "@slintlu";`.
pub const NAME: &str = "slintlu";

/// `$XDG_DATA_HOME/slintlu/lib/<version>`.
pub fn dir() -> PathBuf {
    crate::config::data_home().join("lib").join(env!("CARGO_PKG_VERSION"))
}

/// Writes the library files (when missing or outdated) and returns the entry file.
pub fn install() -> io::Result<PathBuf> {
    let dir = dir();
    std::fs::create_dir_all(&dir)?;
    for (name, contents) in FILES {
        let path = dir.join(name);
        if std::fs::read_to_string(&path).ok().as_deref() != Some(*contents) {
            std::fs::write(&path, contents)?;
        }
    }
    Ok(dir.join(FILES[0].0))
}
