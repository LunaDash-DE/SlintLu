//! How often applications were launched, to rank frequent ones first.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::util::xdg;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LaunchHistory {
    #[serde(default)]
    counts: HashMap<String, u32>,
    #[serde(skip)]
    path: Option<PathBuf>,
}

impl LaunchHistory {
    /// `$XDG_STATE_HOME/slintlu/launches.json`.
    pub fn default_path() -> PathBuf {
        xdg::state_home().join("slintlu").join("launches.json")
    }

    pub fn load(path: PathBuf) -> Self {
        let mut history: Self = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        history.path = Some(path);
        history
    }

    pub fn count(&self, id: &str) -> u32 {
        self.counts.get(id).copied().unwrap_or(0)
    }

    /// Records a launch and saves the history.
    pub fn record(&mut self, id: &str) {
        *self.counts.entry(id.to_owned()).or_default() += 1;
        if let Some(path) = &self.path {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Ok(json) = serde_json::to_string(self)
                && let Err(e) = std::fs::write(path, json)
            {
                log::warn!("cannot save launch history to {}: {e}", path.display());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state/launches.json");
        let mut history = LaunchHistory::load(path.clone());
        history.record("firefox.desktop");
        history.record("firefox.desktop");
        assert_eq!(history.count("firefox.desktop"), 2);
        assert_eq!(LaunchHistory::load(path).count("firefox.desktop"), 2);
    }
}
