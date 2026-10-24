//! The user's language from the environment.

/// `"de_DE.UTF-8@euro"` → `"de_DE"`; empty and POSIX locales give `None`.
pub fn normalize_language(locale: &str) -> Option<String> {
    let base = locale.split(['.', '@']).next().unwrap_or_default().trim();
    match base {
        "" | "C" | "POSIX" => None,
        other => Some(other.to_owned()),
    }
}

/// The first language from `LANGUAGE`, `LC_ALL`, `LC_MESSAGES` or `LANG`.
pub fn current_language() -> Option<String> {
    language_from(|name| std::env::var(name).ok())
}

fn language_from(var: impl Fn(&str) -> Option<String>) -> Option<String> {
    if let Some(list) = var("LANGUAGE")
        && let Some(first) = list.split(':').find_map(normalize_language)
    {
        return Some(first);
    }
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|name| var(name).and_then(|v| normalize_language(&v)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_locales() {
        assert_eq!(normalize_language("de_DE.UTF-8@euro").as_deref(), Some("de_DE"));
        assert_eq!(normalize_language("zh_TW.UTF-8").as_deref(), Some("zh_TW"));
        assert_eq!(normalize_language("C.UTF-8"), None);
        assert_eq!(normalize_language("POSIX"), None);
    }

    #[test]
    fn language_precedence() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| pairs.iter().find(|(k, _)| *k == name).map(|(_, v)| v.to_string())
        };
        assert_eq!(
            language_from(env(&[("LANGUAGE", "fr:de"), ("LANG", "en_US.UTF-8")])).as_deref(),
            Some("fr")
        );
        assert_eq!(
            language_from(env(&[("LC_ALL", "ja_JP.UTF-8"), ("LANG", "en_US")])).as_deref(),
            Some("ja_JP")
        );
        assert_eq!(language_from(env(&[("LANG", "C")])), None);
    }
}
