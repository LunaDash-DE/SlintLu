//! Icon lookup following the freedesktop Icon Theme Specification: theme inheritance, size
//! matching, `hicolor` and `/usr/share/pixmaps` fallbacks.

mod theme;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::util::{ini::Ini, xdg};
use theme::Theme;

/// Formats shells can display (Slint decodes SVG, PNG and JPEG; legacy XPM icons are skipped).
const EXTENSIONS: [&str; 2] = ["svg", "png"];

/// Resolves icon names to files. Results are cached; create one and share it.
#[derive(Debug)]
pub struct IconLookup {
    themes: Vec<Theme>,
    pixmap_dirs: Vec<PathBuf>,
    cache: Mutex<HashMap<(String, u32), Option<PathBuf>>>,
}

fn base_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![xdg::data_home().join("icons"), xdg::home_dir().join(".icons")];
    dirs.extend(xdg::data_dirs().into_iter().map(|d| d.join("icons")));
    dirs.retain(|d| d.is_dir());
    dirs
}

/// The icon theme the desktop is configured with: `$SLINTLU_ICON_THEME`, GTK settings, KDE
/// settings, or `hicolor`.
pub fn default_theme_name() -> String {
    if let Ok(theme) = std::env::var("SLINTLU_ICON_THEME")
        && !theme.is_empty()
    {
        return theme;
    }
    let config = xdg::config_home();
    for (file, group, key) in [
        ("gtk-4.0/settings.ini", "Settings", "gtk-icon-theme-name"),
        ("gtk-3.0/settings.ini", "Settings", "gtk-icon-theme-name"),
        ("kdeglobals", "Icons", "Theme"),
    ] {
        if let Ok(source) = std::fs::read_to_string(config.join(file))
            && let Some(theme) = Ini::parse(&source).get(group, key)
            && !theme.is_empty()
        {
            return theme.trim_matches('"').to_owned();
        }
    }
    "hicolor".into()
}

impl IconLookup {
    /// Uses `theme` (or [`default_theme_name`]) and the standard search directories.
    pub fn new(theme: Option<&str>) -> Self {
        let mut pixmap_dirs = vec![PathBuf::from("/usr/share/pixmaps")];
        pixmap_dirs.extend(xdg::data_dirs().into_iter().map(|d| d.join("pixmaps")));
        pixmap_dirs.dedup();
        Self::with_dirs(
            &theme.map(str::to_owned).unwrap_or_else(default_theme_name),
            &base_dirs(),
            pixmap_dirs,
        )
    }

    fn with_dirs(theme: &str, bases: &[PathBuf], pixmap_dirs: Vec<PathBuf>) -> Self {
        Self { themes: theme::chain(theme, bases), pixmap_dirs, cache: Mutex::default() }
    }

    /// The best file for `name` at `size` logical pixels. `name` may also be an absolute path
    /// or a `file://` URL.
    pub fn lookup(&self, name: &str, size: u32) -> Option<PathBuf> {
        if name.is_empty() {
            return None;
        }
        if let Some(path) =
            name.strip_prefix("file://").or_else(|| name.starts_with('/').then_some(name))
        {
            let path = Path::new(path);
            return path.is_file().then(|| path.to_path_buf());
        }
        let key = (name.to_owned(), size);
        if let Some(hit) = self.cache.lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
            return hit.clone();
        }
        // Some applications put the extension in the name.
        let bare =
            EXTENSIONS.iter().find_map(|ext| name.strip_suffix(&format!(".{ext}"))).unwrap_or(name);
        let found = self
            .themes
            .iter()
            .find_map(|theme| theme.find(bare, size, &EXTENSIONS))
            .or_else(|| self.find_pixmap(bare));
        self.cache.lock().unwrap_or_else(|e| e.into_inner()).insert(key, found.clone());
        found
    }

    fn find_pixmap(&self, name: &str) -> Option<PathBuf> {
        self.pixmap_dirs.iter().find_map(|dir| {
            EXTENSIONS.iter().map(|ext| dir.join(format!("{name}.{ext}"))).find(|p| p.is_file())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "").unwrap();
    }

    fn setup() -> (tempfile::TempDir, IconLookup) {
        let root = tempfile::tempdir().unwrap();
        let icons = root.path().join("icons");
        std::fs::create_dir_all(icons.join("Papirus")).unwrap();
        std::fs::write(
            icons.join("Papirus/index.theme"),
            "[Icon Theme]\nName=Papirus\nInherits=breeze\nDirectories=48x48/apps,scalable/apps\n\
             [48x48/apps]\nSize=48\nType=Fixed\n[scalable/apps]\nSize=64\nMinSize=16\nMaxSize=512\nType=Scalable\n",
        )
        .unwrap();
        touch(&icons.join("Papirus/48x48/apps/firefox.png"));
        touch(&icons.join("Papirus/scalable/apps/firefox.svg"));
        std::fs::create_dir_all(icons.join("breeze")).unwrap();
        std::fs::write(
            icons.join("breeze/index.theme"),
            "[Icon Theme]\nDirectories=apps/32\n[apps/32]\nSize=32\nType=Threshold\n",
        )
        .unwrap();
        touch(&icons.join("breeze/apps/32/kate.png"));
        std::fs::create_dir_all(icons.join("hicolor")).unwrap();
        std::fs::write(
            icons.join("hicolor/index.theme"),
            "[Icon Theme]\nDirectories=16x16/apps\n[16x16/apps]\nSize=16\nType=Fixed\n",
        )
        .unwrap();
        touch(&icons.join("hicolor/16x16/apps/tiny.png"));
        touch(&root.path().join("pixmaps/legacy.png"));
        let lookup = IconLookup::with_dirs("Papirus", &[icons], vec![root.path().join("pixmaps")]);
        (root, lookup)
    }

    #[test]
    fn exact_sizes_win_then_scalable() {
        let (_root, lookup) = setup();
        assert!(lookup.lookup("firefox", 48).unwrap().ends_with("48x48/apps/firefox.png"));
        assert!(lookup.lookup("firefox", 128).unwrap().ends_with("scalable/apps/firefox.svg"));
    }

    #[test]
    fn inheritance_hicolor_and_pixmaps() {
        let (_root, lookup) = setup();
        assert!(lookup.lookup("kate", 48).unwrap().ends_with("breeze/apps/32/kate.png"));
        assert!(lookup.lookup("tiny", 48).unwrap().ends_with("hicolor/16x16/apps/tiny.png"));
        assert!(lookup.lookup("legacy", 48).unwrap().ends_with("pixmaps/legacy.png"));
        assert!(lookup.lookup("kate.png", 48).is_some());
        assert_eq!(lookup.lookup("missing", 48), None);
        assert_eq!(lookup.lookup("", 48), None);
    }

    #[test]
    fn paths_are_accepted() {
        let (root, lookup) = setup();
        let file = root.path().join("pixmaps/legacy.png");
        assert_eq!(lookup.lookup(file.to_str().unwrap(), 48), Some(file.clone()));
        assert_eq!(lookup.lookup(&format!("file://{}", file.display()), 48), Some(file));
        assert_eq!(lookup.lookup("/no/such/file.png", 48), None);
    }
}
