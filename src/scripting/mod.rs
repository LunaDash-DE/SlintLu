//! Bridge between the Slint interpreter and the Wayland platform: compiles the user's shell,
//! turns its windows into layer surfaces or toplevels and feeds it data.

mod bridge;
mod compiler;
pub mod globals;
pub mod host;
pub mod ipc;
mod json;
pub mod library;
mod panel;

pub use compiler::compile;
pub use host::ShellHost;
