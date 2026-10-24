//! The `Theme` global (XDG desktop portal appearance settings).

use std::rc::Rc;

use slint::Color;
use slint_interpreter::Value;
use slintlu::dbus::portal::{Appearance, ColorScheme, Portal};

use super::worker::spawn_watcher;
use crate::scripting::{ShellHost, host};

fn publish(host: &ShellHost, appearance: &Appearance) {
    host.set_global("Theme", "dark", Value::Bool(appearance.color_scheme != ColorScheme::Light));
    host.set_global(
        "Theme",
        "has-preference",
        Value::Bool(appearance.color_scheme != ColorScheme::NoPreference),
    );
    host.set_global("Theme", "high-contrast", Value::Bool(appearance.high_contrast));
    if let Some((r, g, b)) = appearance.accent {
        let channel = |c: f32| (c * 255.0).round() as u8;
        let color = Color::from_rgb_u8(channel(r), channel(g), channel(b));
        host.set_global("Theme", "accent", Value::Brush(color.into()));
    }
}

pub fn install(_host: &Rc<ShellHost>) {
    let portal = match Portal::connect() {
        Ok(portal) => portal,
        Err(e) => return log::info!("theme: no session bus ({e})"),
    };
    spawn_watcher("theme", move || {
        portal
            .watch(|appearance| {
                let appearance = *appearance;
                host::post(move |host| publish(host, &appearance));
            })
            .map_err(std::io::Error::other)
    });
}
