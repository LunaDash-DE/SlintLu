//! A minimal parser for the INI-like files of freedesktop specifications (desktop entries,
//! icon theme indexes, GTK settings).

use std::collections::HashMap;

/// Groups (`[Name]`) mapping keys to raw values, in file order of groups.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ini {
    groups: Vec<(String, HashMap<String, String>)>,
}

impl Ini {
    pub fn parse(source: &str) -> Self {
        let mut groups: Vec<(String, HashMap<String, String>)> = Vec::new();
        for line in source.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                groups.push((name.to_owned(), HashMap::new()));
            } else if let (Some((key, value)), Some((_, group))) =
                (line.split_once('='), groups.last_mut())
            {
                // The first occurrence of a key wins.
                group.entry(key.trim().to_owned()).or_insert_with(|| value.trim().to_owned());
            }
        }
        Self { groups }
    }

    pub fn group(&self, name: &str) -> Option<&HashMap<String, String>> {
        self.groups.iter().find(|(n, _)| n == name).map(|(_, g)| g)
    }

    pub fn get(&self, group: &str, key: &str) -> Option<&str> {
        self.group(group)?.get(key).map(String::as_str)
    }

    pub fn group_names(&self) -> impl Iterator<Item = &str> {
        self.groups.iter().map(|(n, _)| n.as_str())
    }
}

/// Splits a `;`-separated list (`a;b;c;`), honouring `\;` escapes.
pub fn list(value: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut escaped: bool = false;
    for c in value.chars() {
        if escaped {
            match c {
                ';' => current.push(';'),
                _ => {
                    current.push('\\');
                    current.push(c);
                }
            }
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == ';' {
            items.push(std::mem::take(&mut current));
        } else {
            current.push(c);
        }
    }

    if escaped {
        current.push('\\');
    }
    items.push(current);
    items.into_iter().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_groups_and_keys() {
        let ini = Ini::parse(
            "# c\n[Desktop Entry]\nName=Foo\nName=Ignored\n Exec = foo %U \n[Desktop Action new]\nName=New\n",
        );
        assert_eq!(ini.get("Desktop Entry", "Name"), Some("Foo"));
        assert_eq!(ini.get("Desktop Entry", "Exec"), Some("foo %U"));
        assert_eq!(ini.get("Desktop Action new", "Name"), Some("New"));
        assert_eq!(
            ini.group_names().collect::<Vec<_>>(),
            vec!["Desktop Entry", "Desktop Action new"]
        );
        assert_eq!(ini.get("Missing", "x"), None);
    }

    #[test]
    fn splits_lists() {
        assert_eq!(list("Audio;Video;"), vec!["Audio", "Video"]);
        assert_eq!(list(r"a\;b;c"), vec!["a;b", "c"]);
        assert!(list("").is_empty());
    }
}
