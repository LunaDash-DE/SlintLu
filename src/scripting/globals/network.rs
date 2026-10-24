//! The `Network` global (NetworkManager).

use std::rc::Rc;

use slint_interpreter::Value;
use slintlu::dbus::network::{NetworkMonitor, NetworkState};

use super::worker::{run_action, spawn_watcher};
use crate::scripting::bridge::{bool_arg, string};
use crate::scripting::{ShellHost, host};

fn publish(host: &ShellHost, state: &NetworkState) {
    host.set_global("Network", "online", Value::Bool(state.online));
    host.set_global("Network", "connected", Value::Bool(state.connected));
    host.set_global("Network", "kind", string(state.kind.as_str()));
    host.set_global("Network", "name", string(&state.name));
    host.set_global("Network", "strength", Value::Number(state.strength.into()));
    host.set_global("Network", "wifi-enabled", Value::Bool(state.wifi_enabled));
}

pub fn install(host: &Rc<ShellHost>) {
    let monitor = match NetworkMonitor::connect() {
        Ok(monitor) => monitor,
        Err(e) => {
            log::info!("network: NetworkManager unavailable ({e})");
            host.set_global("Network", "available", Value::Bool(false));
            return;
        }
    };
    host.set_global("Network", "available", Value::Bool(true));

    let watcher = monitor.clone();
    spawn_watcher("network", move || {
        watcher
            .watch(|state| {
                let state = state.clone();
                host::post(move |host| publish(host, &state));
            })
            .map_err(std::io::Error::other)
    });

    host.on_callback(
        "Network",
        "set-wifi-enabled",
        Rc::new(move |args| {
            if let Some(enabled) = bool_arg(args, 0) {
                let monitor = monitor.clone();
                run_action("network", move || {
                    monitor.set_wifi_enabled(enabled).map_err(std::io::Error::other)
                });
            }
            Value::Void
        }),
    );
}
