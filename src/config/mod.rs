//! Where SlintLu finds the user's shell and keeps its own files.

mod paths;
mod watch;

pub use paths::{data_home, default_config_dir, resolve_config};
pub use watch::Watcher;
