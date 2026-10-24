//! Turning a directory of `.po` files into a gettext locale tree.

use std::io;
use std::path::{Path, PathBuf};

use super::{mo, po};

/// The result of [`compile_catalogs`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CompiledCatalogs {
    /// Directory to pass to `bindtextdomain`.
    pub locale_dir: PathBuf,
    /// Languages found, e.g. `["de", "zh_TW"]`.
    pub languages: Vec<String>,
}

/// Compiles `<source>/<lang>.po` (or `<source>/<lang>/LC_MESSAGES/*.po`) into
/// `<target>/<lang>/LC_MESSAGES/<domain>.mo`.
pub fn compile_catalogs(
    source: &Path,
    target: &Path,
    domain: &str,
) -> io::Result<CompiledCatalogs> {
    let mut languages = Vec::new();
    for entry in std::fs::read_dir(source)? {
        let path = entry?.path();
        let (language, po_file) = if path.extension().is_some_and(|e| e == "po") {
            let Some(stem) = path.file_stem() else { continue };
            (stem.to_string_lossy().into_owned(), path.clone())
        } else if path.is_dir() {
            let messages = path.join("LC_MESSAGES");
            let Some(po_file) = std::fs::read_dir(&messages)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .find(|p| p.extension().is_some_and(|e| e == "po"))
            else {
                continue;
            };
            (path.file_name().unwrap_or_default().to_string_lossy().into_owned(), po_file)
        } else {
            continue;
        };

        let messages = po::parse_po(&std::fs::read_to_string(&po_file)?);
        let out_dir = target.join(&language).join("LC_MESSAGES");
        std::fs::create_dir_all(&out_dir)?;
        std::fs::write(out_dir.join(format!("{domain}.mo")), mo::write_mo(&messages))?;
        languages.push(language);
    }
    languages.sort();
    Ok(CompiledCatalogs { locale_dir: target.to_path_buf(), languages })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_both_layouts() {
        let source = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        std::fs::write(source.path().join("de.po"), "msgid \"Hi\"\nmsgstr \"Hallo\"\n").unwrap();
        let nested = source.path().join("zh_TW/LC_MESSAGES");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("shell.po"), "msgid \"Hi\"\nmsgstr \"你好\"\n").unwrap();
        std::fs::write(source.path().join("README"), "ignored").unwrap();

        let compiled = compile_catalogs(source.path(), target.path(), "slintlu").unwrap();
        assert_eq!(compiled.languages, vec!["de", "zh_TW"]);
        assert!(target.path().join("de/LC_MESSAGES/slintlu.mo").is_file());
        assert!(target.path().join("zh_TW/LC_MESSAGES/slintlu.mo").is_file());
    }
}
