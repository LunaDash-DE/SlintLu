//! Control a running `slintlu` from scripts, keybindings or other programs.
//!
//! The protocol is one JSON [`Request`] per line on a Unix socket, answered by one JSON
//! [`Response`] line. Any global exported by the shell can be read, written or called, so a
//! compositor keybinding like `slintlu msg toggle Launcher.open` can open a launcher.

mod client;
mod protocol;

pub use client::{Client, socket_path};
pub use protocol::{Request, Response, Target};
