//! Native implementations of the `@slintlu` globals. Each module wires one domain (clock,
//! audio, workspaces …) into the [`ShellHost`](super::ShellHost).

mod shell;

use std::rc::Rc;

use super::ShellHost;

/// Static information the `Shell` global exposes.
pub struct ShellInfo {
    pub compositor: slintlu::compositors::Compositor,
}

/// Registers every global with the host. Call before the first [`ShellHost::load`].
pub fn install(host: &Rc<ShellHost>, info: &ShellInfo) {
    shell::install(host, info);
}
