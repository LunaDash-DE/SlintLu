//! Native implementations of the `@slintlu` globals. Each module wires one domain (clock,
//! audio, workspaces …) into the [`ShellHost`](super::ShellHost).

mod apps;
mod audio;
mod clock;
mod i18n;
mod icons;
mod idle;
mod media;
mod network;
mod notifications;
mod power;
mod session;
mod shell;
mod system;
mod theme;
mod worker;
mod workspaces;

use std::rc::Rc;

use super::ShellHost;

pub use i18n::{TRANSLATION_DOMAIN, load_translations};

/// Static information the `Shell` global exposes.
pub struct ShellInfo {
    pub compositor: slintlu::compositors::Compositor,
}

/// Registers every global with the host. Call before the first [`ShellHost::load`].
pub fn install(host: &Rc<ShellHost>, info: &ShellInfo) {
    shell::install(host, info);
    clock::install(host);
    system::install(host);
    audio::install(host);
    workspaces::install(host, info.compositor);
    notifications::install(host);
    media::install(host);
    network::install(host);
    power::install(host);
    theme::install(host);
    i18n::install(host);
    icons::install(host);
    apps::install(host);
    session::install(host);
    idle::install(host);
}

/// Releases state that must not outlive the event loop.
pub fn shutdown() {
    notifications::shutdown();
    apps::shutdown();
    icons::shutdown();
    idle::shutdown();
}

/// Fills per-window values (such as the formatted time) right after windows are created.
pub fn refresh(host: &ShellHost) {
    clock::refresh(host);
}
