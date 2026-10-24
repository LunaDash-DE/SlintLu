//! The `Power` global (systemd-logind).

use std::rc::Rc;

use slint_interpreter::Value;
use slintlu::dbus::power::{Power, PowerAction};

use super::worker::run_action;
use crate::scripting::ShellHost;

pub fn install(host: &Rc<ShellHost>) {
    let power = match Power::connect() {
        Ok(power) => power,
        Err(e) => {
            log::info!("power: logind unavailable ({e})");
            host.set_global("Power", "available", Value::Bool(false));
            return;
        }
    };
    let (can_suspend, can_hibernate) = power.capabilities();
    host.set_global("Power", "available", Value::Bool(true));
    host.set_global("Power", "can-suspend", Value::Bool(can_suspend));
    host.set_global("Power", "can-hibernate", Value::Bool(can_hibernate));

    for (callback, action) in [
        ("power-off", PowerAction::PowerOff),
        ("reboot", PowerAction::Reboot),
        ("suspend", PowerAction::Suspend),
        ("hibernate", PowerAction::Hibernate),
        ("log-out", PowerAction::LogOut),
        ("lock", PowerAction::Lock),
    ] {
        let power = power.clone();
        host.on_callback(
            "Power",
            callback,
            Rc::new(move |_| {
                let power = power.clone();
                run_action("power", move || power.perform(action).map_err(std::io::Error::other));
                Value::Void
            }),
        );
    }
}
