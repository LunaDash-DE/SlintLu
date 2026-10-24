//! Finding desktop entries in the XDG application directories.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::entry::AppEntry;
use crate::util::xdg;

/// `$XDG_DATA_HOME/applications` followed by `$XDG_DATA_DIRS/*/applications`, highest
/// precedence first.
pub fn application_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![xdg::data_home().join("applications")];
    dirs.extend(xdg::data_dirs().into_iter().map(|d| d.join("applications")));
    dirs.dedup();
    dirs
}

fn visit(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            visit(root, &path, out);
        } else if path.extension().is_some_and(|e| e == "desktop") {
            // The desktop file id replaces directory separators with dashes.
            let id = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('/', "-");
            out.push((id, path));
        }
    }
}

/// All visible applications, sorted by name. Earlier directories shadow later ones with the
/// same desktop file id (so user overrides and hidden entries work).
pub fn scan(dirs: &[PathBuf], language: Option<&str>) -> Vec<AppEntry> {
    let mut seen = HashSet::new();
    let mut apps = Vec::new();
    for dir in dirs {
        let mut files = Vec::new();
        visit(dir, dir, &mut files);
        for (id, path) in files {
            if !seen.insert(id.clone()) {
                continue;
            }
            let Ok(source) = std::fs::read_to_string(&path) else { continue };
            if let Some(app) = AppEntry::parse(&id, &path, &source, language) {
                apps.push(app);
            }
        }
    }
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, body: &str) {
        std::fs::create_dir_all(dir.join(name).parent().unwrap()).unwrap();
        std::fs::write(dir.join(name), format!("[Desktop Entry]\nType=Application\n{body}"))
            .unwrap();
    }

    #[test]
    fn user_entries_shadow_system_ones() {
        let user = tempfile::tempdir().unwrap();
        let system = tempfile::tempdir().unwrap();
        write(system.path(), "editor.desktop", "Name=System Editor\nExec=ed");
        write(system.path(), "kde/konsole.desktop", "Name=Konsole\nExec=konsole");
        write(system.path(), "hidden.desktop", "Name=Hidden\nExec=x");
        write(user.path(), "editor.desktop", "Name=My Editor\nExec=vim");
        write(user.path(), "hidden.desktop", "Name=Hidden\nExec=x\nHidden=true");

        let apps = scan(&[user.path().to_path_buf(), system.path().to_path_buf()], None);
        let names: Vec<_> = apps.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, vec!["Konsole", "My Editor"]);
        assert_eq!(apps[0].id, "kde-konsole.desktop");
    }
}
