//! Icon theme indexes (`index.theme`) and size matching.

use std::path::{Path, PathBuf};

use crate::util::ini::{Ini, list};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Fixed,
    Scalable { min: u32, max: u32 },
    Threshold(u32),
}

#[derive(Clone, Debug)]
struct Directory {
    path: String,
    size: u32,
    scale: u32,
    kind: Kind,
}

impl Directory {
    fn matches(&self, size: u32) -> bool {
        match self.kind {
            Kind::Fixed => self.size == size,
            Kind::Scalable { min, max } => (min..=max).contains(&size),
            Kind::Threshold(t) => self.size.abs_diff(size) <= t,
        }
    }

    fn distance(&self, size: u32) -> u32 {
        match self.kind {
            Kind::Fixed => self.size.abs_diff(size),
            Kind::Scalable { min, max } => min.saturating_sub(size).max(size.saturating_sub(max)),
            Kind::Threshold(t) => self.size.abs_diff(size).saturating_sub(t),
        }
    }
}

/// One theme: its directories, and every base directory it was found in.
#[derive(Clone, Debug)]
pub struct Theme {
    name: String,
    bases: Vec<PathBuf>,
    directories: Vec<Directory>,
}

fn load(name: &str, bases: &[PathBuf]) -> Option<(Theme, Vec<String>)> {
    let roots: Vec<PathBuf> = bases.iter().map(|b| b.join(name)).filter(|p| p.is_dir()).collect();
    let index = roots.iter().find_map(|r| std::fs::read_to_string(r.join("index.theme")).ok())?;
    let ini = Ini::parse(&index);
    let header = "Icon Theme";
    let mut names = list(&ini.get(header, "Directories").unwrap_or_default().replace(',', ";"));
    names.extend(list(&ini.get(header, "ScaledDirectories").unwrap_or_default().replace(',', ";")));
    let directories = names
        .into_iter()
        .filter_map(|path| {
            let number = |key: &str| ini.get(&path, key).and_then(|v| v.parse::<u32>().ok());
            let size = number("Size")?;
            let kind = match ini.get(&path, "Type").unwrap_or("Threshold") {
                "Fixed" => Kind::Fixed,
                "Scalable" => Kind::Scalable {
                    min: number("MinSize").unwrap_or(size),
                    max: number("MaxSize").unwrap_or(size),
                },
                _ => Kind::Threshold(number("Threshold").unwrap_or(2)),
            };
            Some(Directory { size, scale: number("Scale").unwrap_or(1), kind, path })
        })
        .collect();
    let inherits = list(&ini.get(header, "Inherits").unwrap_or_default().replace(',', ";"));
    Some((Theme { name: name.to_owned(), bases: roots, directories }, inherits))
}

/// `theme` followed by everything it inherits (depth first), always ending with `hicolor`.
pub fn chain(theme: &str, bases: &[PathBuf]) -> Vec<Theme> {
    let mut themes: Vec<Theme> = Vec::new();
    let mut pending = vec![theme.to_owned()];
    while let Some(name) = pending.pop() {
        if themes.iter().any(|t| t.name == name) || themes.len() > 32 {
            continue;
        }
        if let Some((theme, inherits)) = load(&name, bases) {
            themes.push(theme);
            pending.extend(inherits.into_iter().rev());
        }
    }
    if let Some(index) = themes.iter().position(|t| t.name == "hicolor") {
        let hicolor = themes.remove(index);
        themes.push(hicolor);
    } else if let Some((hicolor, _)) = load("hicolor", bases) {
        themes.push(hicolor);
    }
    themes
}

impl Theme {
    fn file(
        &self,
        base: &Path,
        dir: &Directory,
        name: &str,
        extensions: &[&str],
    ) -> Option<PathBuf> {
        extensions
            .iter()
            .map(|ext| base.join(&dir.path).join(format!("{name}.{ext}")))
            .find(|p| p.is_file())
    }

    /// The best file in this theme, or `None`.
    pub fn find(&self, name: &str, size: u32, extensions: &[&str]) -> Option<PathBuf> {
        let unscaled = || self.directories.iter().filter(|d| d.scale == 1);
        for dir in unscaled().filter(|d| d.matches(size)) {
            if let Some(file) = self.bases.iter().find_map(|b| self.file(b, dir, name, extensions))
            {
                return Some(file);
            }
        }
        let mut candidates: Vec<&Directory> = unscaled().collect();
        candidates.sort_by_key(|d| d.distance(size));
        candidates
            .into_iter()
            .find_map(|dir| self.bases.iter().find_map(|b| self.file(b, dir, name, extensions)))
    }
}
