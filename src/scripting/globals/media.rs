//! The `Media` global (MPRIS media players).

use std::rc::Rc;

use slint_interpreter::Value;
use slintlu::dbus::mpris::{MediaCommand, MediaPlayer, MediaState, Mpris};

use super::worker::{run_action, spawn_watcher};
use crate::scripting::bridge::{list_value, number_arg, string, string_arg, struct_value};
use crate::scripting::{ShellHost, host};

fn player(p: Option<&MediaPlayer>) -> Value {
    let p = p.cloned().unwrap_or_default();
    struct_value([
        ("bus-name", string(&p.bus_name)),
        ("identity", string(&p.identity)),
        ("title", string(&p.title)),
        ("artist", string(&p.artist)),
        ("album", string(&p.album)),
        ("art-url", string(&p.art_url)),
        ("playing", Value::Bool(p.playing)),
        ("paused", Value::Bool(p.paused)),
        ("position", Value::Number(p.position)),
        ("length", Value::Number(p.length)),
        ("can-go-next", Value::Bool(p.can_go_next)),
        ("can-go-previous", Value::Bool(p.can_go_previous)),
    ])
}

fn publish(host: &ShellHost, state: &MediaState) {
    host.set_global("Media", "player", player(state.active()));
    host.set_global("Media", "players", list_value(state.players.iter().map(|p| player(Some(p)))));
}

/// The bus name of the player commands without a target go to.
fn active_player(host: &ShellHost) -> Option<String> {
    match host.global_property("Media", "player") {
        Some(Value::Struct(s)) => match s.get_field("bus-name") {
            Some(Value::String(name)) if !name.is_empty() => Some(name.to_string()),
            _ => None,
        },
        _ => None,
    }
}

fn command(
    host: &Rc<ShellHost>,
    mpris: &Mpris,
    name: &str,
    make: fn(&[Value]) -> Option<MediaCommand>,
) {
    let mpris = mpris.clone();
    let weak = Rc::downgrade(host);
    host.on_callback(
        "Media",
        name,
        Rc::new(move |args| {
            if let (Some(command), Some(bus)) =
                (make(args), weak.upgrade().and_then(|h| active_player(&h)))
            {
                let mpris = mpris.clone();
                run_action("media", move || {
                    mpris.command(&bus, command).map_err(std::io::Error::other)
                });
            }
            Value::Void
        }),
    );
}

pub fn install(host: &Rc<ShellHost>) {
    let mpris = match Mpris::connect() {
        Ok(mpris) => mpris,
        Err(e) => {
            log::info!("media: no session bus ({e})");
            host.set_global("Media", "available", Value::Bool(false));
            return;
        }
    };
    host.set_global("Media", "available", Value::Bool(true));

    let watcher = mpris.clone();
    spawn_watcher("media", move || {
        watcher
            .watch(|state| {
                let state = state.clone();
                host::post(move |host| publish(host, &state));
            })
            .map_err(std::io::Error::other)
    });

    command(host, &mpris, "play-pause", |_| Some(MediaCommand::PlayPause));
    command(host, &mpris, "next", |_| Some(MediaCommand::Next));
    command(host, &mpris, "previous", |_| Some(MediaCommand::Previous));
    command(host, &mpris, "seek", |a| Some(MediaCommand::Seek(number_arg(a, 0)? as i64)));
    host.on_callback(
        "Media",
        "command",
        Rc::new(move |args| {
            let command = match string_arg(args, 1).as_deref() {
                Some("play-pause") => Some(MediaCommand::PlayPause),
                Some("next") => Some(MediaCommand::Next),
                Some("previous") => Some(MediaCommand::Previous),
                _ => None,
            };
            if let (Some(bus), Some(command)) = (string_arg(args, 0), command) {
                let mpris = mpris.clone();
                run_action("media", move || {
                    mpris.command(&bus, command).map_err(std::io::Error::other)
                });
            }
            Value::Void
        }),
    );
}
