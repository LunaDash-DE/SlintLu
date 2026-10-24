//! A small persistent key–value store for shell settings (wallpaper, toggles, positions …),
//! saved as JSON.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

#[derive(Debug, Default)]
pub struct Store {
    path: Option<PathBuf>,
    values: BTreeMap<String, Value>,
}

impl Store {
    /// Loads `path` (an empty store when it does not exist or is broken).
    pub fn load(path: &Path) -> Self {
        let values = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        Self { path: Some(path.to_path_buf()), values }
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.values.get(key)
    }

    /// Sets a value and writes the file (atomically, through a temporary file).
    pub fn set(&mut self, key: &str, value: Value) {
        if self.values.get(key) == Some(&value) {
            return;
        }
        self.values.insert(key.to_owned(), value);
        self.save();
    }

    pub fn remove(&mut self, key: &str) {
        if self.values.remove(key).is_some() {
            self.save();
        }
    }

    fn save(&self) {
        let Some(path) = &self.path else { return };
        let result = (|| -> std::io::Result<()> {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let temporary = path.with_extension("json.tmp");
            std::fs::write(
                &temporary,
                serde_json::to_vec_pretty(&self.values).map_err(std::io::Error::other)?,
            )?;
            std::fs::rename(temporary, path)
        })();
        if let Err(e) = result {
            log::warn!("cannot save {}: {e}", path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn values_persist() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state/store.json");
        let mut store = Store::load(&path);
        store.set("wallpaper", json!("/w.png"));
        store.set("dock", json!(true));
        store.set("scale", json!(1.5));
        let reloaded = Store::load(&path);
        assert_eq!(reloaded.get("wallpaper"), Some(&json!("/w.png")));
        assert_eq!(reloaded.get("dock"), Some(&json!(true)));
        let mut reloaded = reloaded;
        reloaded.remove("dock");
        assert_eq!(Store::load(&path).get("dock"), None);
    }

    #[test]
    fn broken_files_start_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.json");
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(Store::load(&path).get("x"), None);
    }
}
