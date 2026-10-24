//! The `Brightness` and `Bluetooth` globals.

use std::rc::Rc;
use std::time::Duration;

use slint::{Timer, TimerMode};
use slint_interpreter::Value;
use slintlu::dbus::bluetooth::{Bluetooth, BluetoothState};
use slintlu::dbus::power::Power;
use slintlu::system;

use super::worker::{run_action, spawn_watcher};
use crate::scripting::bridge::{
    bool_arg, list_value, number_arg, string, string_arg, struct_value,
};
use crate::scripting::{ShellHost, host};

const BRIGHTNESS_POLL: Duration = Duration::from_secs(2);

fn publish_brightness(host: &ShellHost) {
    let light = system::backlight();
    host.set_global("Brightness", "available", Value::Bool(light.is_some()));
    if let Some(light) = light {
        host.set_global("Brightness", "level", Value::Number(light.level().into()));
    }
}

fn install_brightness(host: &Rc<ShellHost>) {
    publish_brightness(host);
    let weak = Rc::downgrade(host);
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, BRIGHTNESS_POLL, move || {
        if let Some(host) = weak.upgrade() {
            publish_brightness(&host);
        }
    });
    std::mem::forget(timer);

    host.on_callback(
        "Brightness",
        "set",
        Rc::new(|args| {
            let (Some(level), Some(light)) = (number_arg(args, 0), system::backlight()) else {
                return Value::Void;
            };
            let raw = light.raw_for(level as f32);
            run_action("brightness", move || {
                Power::connect()
                    .and_then(|p| p.set_brightness("backlight", &light.name, raw))
                    .map_err(std::io::Error::other)?;
                host::post(|host| publish_brightness(host));
                Ok(())
            });
            Value::Void
        }),
    );
}

fn publish_bluetooth(host: &ShellHost, state: &BluetoothState) {
    host.set_global("Bluetooth", "available", Value::Bool(state.available));
    host.set_global("Bluetooth", "powered", Value::Bool(state.powered));
    host.set_global(
        "Bluetooth",
        "connected-count",
        Value::Number(state.devices.iter().filter(|d| d.connected).count() as f64),
    );
    host.set_global(
        "Bluetooth",
        "devices",
        list_value(state.devices.iter().map(|d| {
            struct_value([
                ("path", string(&d.path)),
                ("address", string(&d.address)),
                ("name", string(&d.name)),
                ("icon", string(&d.icon)),
                ("connected", Value::Bool(d.connected)),
                ("paired", Value::Bool(d.paired)),
                ("battery", Value::Number(d.battery.map_or(-1.0, f64::from))),
            ])
        })),
    );
}

fn install_bluetooth(host: &Rc<ShellHost>) {
    let bluetooth = match Bluetooth::connect() {
        Ok(bluetooth) => bluetooth,
        Err(e) => return log::info!("bluetooth: no system bus ({e})"),
    };
    let watcher = bluetooth.clone();
    spawn_watcher("bluetooth", move || {
        watcher
            .watch(|state| {
                let state = state.clone();
                host::post(move |host| publish_bluetooth(host, &state));
            })
            .map_err(std::io::Error::other)
    });

    let b = bluetooth.clone();
    host.on_callback(
        "Bluetooth",
        "set-powered",
        Rc::new(move |args| {
            if let Some(powered) = bool_arg(args, 0) {
                let b = b.clone();
                run_action("bluetooth", move || {
                    b.set_powered(powered).map_err(std::io::Error::other)
                });
            }
            Value::Void
        }),
    );
    for (callback, connected) in [("connect", true), ("disconnect", false)] {
        let b = bluetooth.clone();
        host.on_callback(
            "Bluetooth",
            callback,
            Rc::new(move |args| {
                if let Some(path) = string_arg(args, 0) {
                    let b = b.clone();
                    run_action("bluetooth", move || {
                        b.set_connected(&path, connected).map_err(std::io::Error::other)
                    });
                }
                Value::Void
            }),
        );
    }
}

pub fn install(host: &Rc<ShellHost>) {
    install_brightness(host);
    install_bluetooth(host);
}
