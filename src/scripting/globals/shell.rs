//! The `Shell` global: environment information and process-level actions.

use std::process::{Command, Stdio};
use std::rc::Rc;

use slint_interpreter::Value;

use super::ShellInfo;
use crate::scripting::ShellHost;
use crate::scripting::bridge::{string, string_arg};
use crate::scripting::host;

pub fn install(host: &Rc<ShellHost>, info: &ShellInfo) {
    host.set_global("Shell", "compositor", string(info.compositor.as_str()));
    host.set_global("Shell", "version", string(env!("CARGO_PKG_VERSION")));

    host.on_callback(
        "Shell",
        "exec",
        Rc::new(|args| {
            if let Some(command) = string_arg(args, 0) {
                spawn_detached(&command);
            }
            Value::Void
        }),
    );
    host.on_callback(
        "Shell",
        "env",
        Rc::new(|args| {
            string(std::env::var(string_arg(args, 0).unwrap_or_default()).unwrap_or_default())
        }),
    );
    host.on_callback(
        "Shell",
        "reload",
        Rc::new(|_| {
            // Never drop the windows from inside one of their own callbacks.
            host::post(|host| {
                host.load();
            });
            Value::Void
        }),
    );
    host.on_callback(
        "Shell",
        "quit",
        Rc::new(|_| {
            let _ = slint::quit_event_loop();
            Value::Void
        }),
    );
}

/// Runs `command` through `sh -c` in its own process group, so it survives the shell.
pub fn spawn_detached(command: &str) {
    use std::os::unix::process::CommandExt;
    let child = Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .process_group(0)
        .spawn();
    match child {
        // Reap the child in the background so it never becomes a zombie.
        Ok(mut child) => {
            std::thread::spawn(move || child.wait());
        }
        Err(e) => log::error!("cannot run `{command}`: {e}"),
    }
}
