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

    action(host, &backend, "focus", |a| Some(Action::FocusWorkspace(number_arg(a, 0)? as u64)));
    action(host, &backend, "focus-window", |a| Some(Action::FocusWindow(window_id(a)?)));
    action(host, &backend, "close-window", |a| Some(Action::CloseWindow(window_id(a)?)));
}
