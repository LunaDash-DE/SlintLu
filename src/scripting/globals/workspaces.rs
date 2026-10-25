//! The `Workspaces` global, backed by the compositor's IPC.

use std::rc::Rc;
use std::sync::Arc;

use slint_interpreter::Value;
use slintlu::compositors::{self, Action, Backend, Compositor, DesktopState, Window};

use super::worker::{run_action, spawn_watcher};
use crate::scripting::bridge::{list_value, number_arg, string, string_arg, struct_value};
use crate::scripting::{ShellHost, host};

fn window(w: Option<&Window>) -> Value {
    let w = w.cloned().unwrap_or_default();
    struct_value([
        ("id", string(w.id.to_string())),
        ("title", string(&w.title)),
        ("app-id", string(&w.app_id)),
        ("workspace", Value::Number(w.workspace.unwrap_or(0) as f64)),
        ("focused", Value::Bool(w.focused)),
        ("urgent", Value::Bool(w.urgent)),
    ])
}

fn publish(host: &ShellHost, state: &DesktopState) {
    let workspaces = state.workspaces.iter().map(|ws| {
        struct_value([
            ("id", Value::Number(ws.id as f64)),
            ("index", Value::Number(ws.index as f64)),
            ("name", string(&ws.name)),
            ("output", string(&ws.output)),
            ("active", Value::Bool(ws.active)),
            ("focused", Value::Bool(ws.focused)),
            ("urgent", Value::Bool(ws.urgent)),
            ("windows", Value::Number(ws.windows.into())),
        ])
    });
    host.set_global("Workspaces", "all", list_value(workspaces));
    host.set_global(
        "Workspaces",
        "windows",
        list_value(state.windows.iter().map(|w| window(Some(w)))),
    );
    host.set_global("Workspaces", "focused-window", window(state.focused_window()));
    host.set_global("Workspaces", "keyboard-layout", string(&state.keyboard_layout));
    let focused_output = state.workspaces.iter().find(|w| w.focused).map(|w| w.output.as_str());
    host.set_global("Workspaces", "focused-output", string(focused_output.unwrap_or_default()));
    if state.session_ending {
        log::info!("the compositor is ending the session: quitting");
        let _ = slint::quit_event_loop();
        return;
    }
    host.set_global("Compositor", "wallpaper", string(&state.wallpaper));
    host.set_global("Compositor", "language", string(&state.language));
    if let Some(request) = &state.request {
        host.set_global("Compositor", "request", string(&request.action));
        host.set_global("Compositor", "request-argument", string(&request.argument));
        // Set last: shells react to the serial changing.
        host.set_global("Compositor", "request-serial", Value::Number(request.serial as f64));
    }
}

fn action(
    host: &ShellHost,
    backend: &Arc<dyn Backend>,
    callback: &str,
    make: fn(&[Value]) -> Option<Action>,
) {
    let backend = backend.clone();
    host.on_callback(
        "Workspaces",
        callback,
        Rc::new(move |args| {
            if let Some(action) = make(args) {
                let backend = backend.clone();
                run_action("workspaces", move || backend.perform(&action));
            }
            Value::Void
        }),
    );
}

fn window_id(args: &[Value]) -> Option<u64> {
    string_arg(args, 0)?.parse().ok()
}

pub fn install(host: &Rc<ShellHost>, compositor: Compositor) {
    host.set_global("Compositor", "name", string(compositor.as_str()));
    host.set_global("Compositor", "has-commands", Value::Bool(compositor == Compositor::LunaDash));
    let Some(backend) = compositors::backend(compositor) else {
        log::info!("workspaces: no IPC support for the {compositor} compositor");
        host.set_global("Workspaces", "available", Value::Bool(false));
        return;
    };
    host.set_global("Workspaces", "available", Value::Bool(true));

    let watcher = backend.clone();
    spawn_watcher("workspaces", move || {
        watcher.watch(&mut |state| {
            let state = state.clone();
            host::post(move |host| publish(host, &state));
        })
    });

    let commands = backend.clone();
    host.on_callback(
        "Compositor",
        "command",
        Rc::new(move |args| {
            if let Some(method) = string_arg(args, 0) {
                let value = string_arg(args, 1).unwrap_or_default();
                let backend = commands.clone();
                run_action("compositor", move || backend.command(&method, &value).map(drop));
            }
            Value::Void
        }),
    );
    action(host, &backend, "focus", |a| Some(Action::FocusWorkspace(number_arg(a, 0)? as u64)));
    action(host, &backend, "focus-window", |a| Some(Action::FocusWindow(window_id(a)?)));
    action(host, &backend, "close-window", |a| Some(Action::CloseWindow(window_id(a)?)));
}
