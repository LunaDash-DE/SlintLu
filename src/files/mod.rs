//! Files for shells: listing directories (e.g. wallpapers) and cached thumbnails.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::util::xdg;

/// A file in a listed directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
}

/// Expands a leading `~` to the home directory; `file://` URIs become paths.
pub fn expand_home(path: &str) -> PathBuf {
    if let Some(path) = crate::util::uri::file_uri_path(path) {
        return path;
    }
    match path.strip_prefix('~') {
        Some(rest) => xdg::home_dir().join(rest.trim_start_matches('/')),
        None => PathBuf::from(path),
    }
}

/// Entries of `dir`, directories first, then by name. `extensions` (lowercase, without dots)
/// filters files; empty keeps all. Hidden files are skipped.
pub fn list(dir: &Path, extensions: &[&str]) -> Vec<FileEntry> {
    let mut entries: Vec<FileEntry> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                return None;
            }
            let is_dir = path.is_dir();
            let wanted = is_dir
                || extensions.is_empty()
                || path
                    .extension()
                    .and_then(|x| x.to_str())
                    .is_some_and(|x| extensions.contains(&x.to_ascii_lowercase().as_str()));
            wanted.then_some(FileEntry { path, name, is_dir })
        })
        .collect();
    entries.sort_by(|a, b| {
        b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    entries
}

fn cache_dir() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| xdg::home_dir().join(".cache"))
        .join("slintlu/thumbnails")
}

/// Where the thumbnail of `path` at `size` is cached; changes when the file changes.
pub fn thumbnail_path(path: &Path, size: u32) -> PathBuf {
    thumbnail_path_in(&cache_dir(), path, size)
}

fn thumbnail_path_in(cache: &Path, path: &Path, size: u32) -> PathBuf {
    let modified = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    let mut hasher = DefaultHasher::new();
    (path, modified, size).hash(&mut hasher);
    cache.join(format!("{:016x}.png", hasher.finish()))
}

/// Creates (or reuses) a PNG thumbnail at most `size` pixels wide or high. Blocking.
pub fn thumbnail(path: &Path, size: u32) -> Result<PathBuf, String> {
    thumbnail_in(&cache_dir(), path, size)
}

fn thumbnail_in(cache: &Path, path: &Path, size: u32) -> Result<PathBuf, String> {
    let target = thumbnail_path_in(cache, path, size);
    if target.is_file() {
        return Ok(target);
    }
    let image = image::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let thumb = image.thumbnail(size, size);
    if let Some(dir) = target.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    // Written next to the target and renamed, so readers never see a half-written file.
    let partial = target.with_extension(format!("{}.part", std::process::id()));
    thumb.save_with_format(&partial, image::ImageFormat::Png).map_err(|e| e.to_string())?;
    std::fs::rename(&partial, &target).map_err(|e| e.to_string())?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_filtered_and_sorted() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["b.PNG", "a.jpg", "notes.txt", ".hidden.png"] {
            std::fs::write(dir.path().join(name), "").unwrap();
        }
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        let names: Vec<_> = list(dir.path(), &["png", "jpg"]).into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["sub", "a.jpg", "b.PNG"]);
    }

    #[test]
    fn thumbnails_are_cached() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("big.png");
        image::RgbImage::from_pixel(400, 200, image::Rgb([200, 10, 10])).save(&source).unwrap();
        let thumb = thumbnail_in(dir.path(), &source, 100).unwrap();
        let decoded = image::open(&thumb).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (100, 50));
        assert_eq!(thumbnail_in(dir.path(), &source, 100).unwrap(), thumb);
    }

    #[test]
    fn expands_home() {
        assert_eq!(expand_home("~/Pictures"), xdg::home_dir().join("Pictures"));
        assert_eq!(expand_home("/abs"), PathBuf::from("/abs"));
    }
}
