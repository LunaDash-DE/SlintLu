//! Desktop entries (`.desktop` files) of type `Application`.

use std::path::{Path, PathBuf};

use crate::util::ini::{Ini, list};

/// An extra action of an application, e.g. "New Private Window".
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AppAction {
    pub id: String,
    pub name: String,
    pub exec: String,
    pub icon: String,
}

/// A launchable application.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AppEntry {
    /// Desktop file id, e.g. `org.mozilla.firefox.desktop`.
    pub id: String,
    pub path: PathBuf,
    pub name: String,
    pub generic_name: String,
    pub comment: String,
    pub icon: String,
    pub exec: String,
    pub terminal: bool,
    pub categories: Vec<String>,
    pub keywords: Vec<String>,
    pub startup_wm_class: String,
    pub actions: Vec<AppAction>,
}

const GROUP: &str = "Desktop Entry";

/// Locale keys to try, most specific first: `de_DE@euro` → `de_DE@euro, de_DE, de@euro, de`.
fn locale_variants(language: Option<&str>) -> Vec<String> {
    let Some(language) = language else { return Vec::new() };
    let (base, modifier) = match language.split_once('@') {
        Some((b, m)) => (b, Some(m)),
        None => (language, None),
    };
    let lang = base.split('_').next().unwrap_or(base);
    let mut variants = Vec::new();
    if let Some(m) = modifier {
        variants.push(format!("{base}@{m}"));
    }
    variants.push(base.to_owned());
    if lang != base {
        if let Some(m) = modifier {
            variants.push(format!("{lang}@{m}"));
        }
        variants.push(lang.to_owned());
    }
    variants
}

fn localized(ini: &Ini, group: &str, key: &str, locales: &[String]) -> String {
    locales
        .iter()
        .find_map(|l| ini.get(group, &format!("{key}[{l}]")))
        .or_else(|| ini.get(group, key))
        .unwrap_or_default()
        .to_owned()
}

/// Desktop names this session reports (`XDG_CURRENT_DESKTOP`).
fn current_desktops() -> Vec<String> {
    std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

fn try_exec_ok(try_exec: &str) -> bool {
    if try_exec.is_empty() {
        return true;
    }
    let path = Path::new(try_exec);
    if path.is_absolute() {
        return path.is_file();
    }
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(try_exec).is_file()))
}

impl AppEntry {
    /// Parses a desktop entry; `None` for hidden, non-application or not-for-this-desktop
    /// entries.
    pub fn parse(id: &str, path: &Path, source: &str, language: Option<&str>) -> Option<Self> {
        Self::parse_for(id, path, source, language, &current_desktops())
    }

    fn parse_for(
        id: &str,
        path: &Path,
        source: &str,
        language: Option<&str>,
        desktops: &[String],
    ) -> Option<Self> {
        let ini = Ini::parse(source);
        let get = |key: &str| ini.get(GROUP, key).unwrap_or_default();
        let truthy = |key: &str| get(key) == "true";
        if get("Type") != "Application"
            || truthy("NoDisplay")
            || truthy("Hidden")
            || get("Exec").is_empty()
        {
            return None;
        }
        let only = list(get("OnlyShowIn"));
        let not = list(get("NotShowIn"));
        if (!only.is_empty() && !only.iter().any(|d| desktops.contains(d)))
            || not.iter().any(|d| desktops.contains(d))
            || !try_exec_ok(get("TryExec"))
        {
            return None;
        }

        let locales = locale_variants(language);
        let actions = list(get("Actions"))
            .into_iter()
            .filter_map(|action| {
                let group = format!("Desktop Action {action}");
                let exec = ini.get(&group, "Exec")?.to_owned();
                Some(AppAction {
                    name: localized(&ini, &group, "Name", &locales),
                    icon: ini.get(&group, "Icon").unwrap_or_default().to_owned(),
                    id: action,
                    exec,
                })
            })
            .collect();
        let mut keywords = list(&localized(&ini, GROUP, "Keywords", &locales));
        for keyword in list(get("Keywords")) {
            if !keywords.contains(&keyword) {
                keywords.push(keyword);
            }
        }
        Some(Self {
            id: id.to_owned(),
            path: path.to_path_buf(),
            name: localized(&ini, GROUP, "Name", &locales),
            generic_name: localized(&ini, GROUP, "GenericName", &locales),
            comment: localized(&ini, GROUP, "Comment", &locales),
            icon: get("Icon").to_owned(),
            exec: get("Exec").to_owned(),
            terminal: truthy("Terminal"),
            categories: list(get("Categories")),
            keywords,
            startup_wm_class: get("StartupWMClass").to_owned(),
            actions,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIREFOX: &str = "[Desktop Entry]\nType=Application\nName=Firefox\nName[de]=Firefox-Browser\n\
        GenericName=Web Browser\nGenericName[zh_TW]=網頁瀏覽器\nIcon=firefox\nExec=firefox %u\n\
        Categories=Network;WebBrowser;\nKeywords=web;internet;\nActions=new-private;\n\
        [Desktop Action new-private]\nName=New Private Window\nExec=firefox --private-window %u\n";

    fn parse(source: &str, language: Option<&str>, desktops: &[&str]) -> Option<AppEntry> {
        let desktops: Vec<String> = desktops.iter().map(|s| s.to_string()).collect();
        AppEntry::parse_for(
            "firefox.desktop",
            Path::new("/x/firefox.desktop"),
            source,
            language,
            &desktops,
        )
    }

    #[test]
    fn parses_localized_fields_and_actions() {
        let app = parse(FIREFOX, Some("de_DE"), &[]).unwrap();
        assert_eq!(app.name, "Firefox-Browser");
        assert_eq!(app.generic_name, "Web Browser");
        assert_eq!(app.categories, vec!["Network", "WebBrowser"]);
        assert_eq!(app.keywords, vec!["web", "internet"]);
        assert_eq!(app.actions[0].name, "New Private Window");
        let app = parse(FIREFOX, Some("zh_TW"), &[]).unwrap();
        assert_eq!(app.generic_name, "網頁瀏覽器");
        assert_eq!(app.name, "Firefox");
    }

    #[test]
    fn filters_hidden_and_desktop_specific_entries() {
        let hidden = FIREFOX.replace("Type=Application", "Type=Application\nNoDisplay=true");
        assert!(parse(&hidden, None, &[]).is_none());
        let gnome_only = FIREFOX.replace("Type=Application", "Type=Application\nOnlyShowIn=GNOME;");
        assert!(parse(&gnome_only, None, &["niri"]).is_none());
        assert!(parse(&gnome_only, None, &["GNOME"]).is_some());
        let not_kde = FIREFOX.replace("Type=Application", "Type=Application\nNotShowIn=KDE;");
        assert!(parse(&not_kde, None, &["KDE"]).is_none());
        assert!(parse(&FIREFOX.replace("Type=Application", "Type=Link"), None, &[]).is_none());
        let missing =
            FIREFOX.replace("Type=Application", "Type=Application\nTryExec=/no/such/binary");
        assert!(parse(&missing, None, &[]).is_none());
    }

    #[test]
    fn locale_fallback_order() {
        assert_eq!(
            locale_variants(Some("de_DE@euro")),
            vec!["de_DE@euro", "de_DE", "de@euro", "de"]
        );
        assert_eq!(locale_variants(Some("fr")), vec!["fr"]);
        assert!(locale_variants(None).is_empty());
    }
}
