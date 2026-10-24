//! The `Audio` global, backed by `pactl`.

use std::rc::Rc;

use slint_interpreter::Value;
use slintlu::audio::{AudioState, Device, DeviceKind, Pactl};

use super::worker::{run_action, spawn_watcher};
use crate::scripting::bridge::{
    bool_arg, list_value, number_arg, string, string_arg, struct_value,
};
use crate::scripting::{ShellHost, host};

fn device(d: Option<&Device>) -> Value {
    let d = d.cloned().unwrap_or_default();
    struct_value([
        ("name", string(&d.name)),
        ("description", string(&d.description)),
        ("volume", Value::Number(d.volume.into())),
        ("muted", Value::Bool(d.muted)),
    ])
}

fn publish(host: &ShellHost, state: &AudioState) {
    host.set_global("Audio", "output", device(state.output()));
    host.set_global("Audio", "input", device(state.input()));
    host.set_global("Audio", "outputs", list_value(state.outputs.iter().map(|d| device(Some(d)))));
    host.set_global("Audio", "inputs", list_value(state.inputs.iter().map(|d| device(Some(d)))));
}

/// Registers a callback that runs a pactl command off the UI thread.
fn action(
    host: &ShellHost,
    callback: &str,
    run: impl Fn(&[Value]) -> Option<Box<dyn FnOnce() -> std::io::Result<()> + Send>> + 'static,
) {
    host.on_callback(
        "Audio",
        callback,
        Rc::new(move |args| {
            if let Some(job) = run(args) {
                run_action("audio", job);
            }
            Value::Void
        }),
    );
}

pub fn install(host: &Rc<ShellHost>) {
    let Some(pactl) = Pactl::detect() else {
        log::info!("audio: no sound server reachable through pactl");
        host.set_global("Audio", "available", Value::Bool(false));
        return;
    };
    host.set_global("Audio", "available", Value::Bool(true));

    spawn_watcher("audio", move || {
        pactl.watch(&mut |state| {
            let state = state.clone();
            host::post(move |host| publish(host, &state));
        })
    });

    use DeviceKind::{Input, Output};
    action(host, "set-volume", move |a| {
        let v = number_arg(a, 0)? as f32;
        Some(Box::new(move || pactl.set_volume(Output, v)))
    });
    action(host, "set-input-volume", move |a| {
        let v = number_arg(a, 0)? as f32;
        Some(Box::new(move || pactl.set_volume(Input, v)))
    });
    action(host, "set-muted", move |a| {
        let muted = bool_arg(a, 0)?;
        Some(Box::new(move || pactl.set_muted(Output, muted)))
    });
    action(host, "toggle-muted", move |_| Some(Box::new(move || pactl.toggle_muted(Output))));
    action(host, "toggle-input-muted", move |_| Some(Box::new(move || pactl.toggle_muted(Input))));
    action(host, "set-default-output", move |a| {
        let name = string_arg(a, 0)?;
        Some(Box::new(move || pactl.set_default(Output, &name)))
    });
    action(host, "set-default-input", move |a| {
        let name = string_arg(a, 0)?;
        Some(Box::new(move || pactl.set_default(Input, &name)))
    });
}
