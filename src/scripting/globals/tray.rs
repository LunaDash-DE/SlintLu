//! The `Tray` global (StatusNotifierItem host).

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use slint_interpreter::Value;
use slintlu::dbus::tray::{Tray, TrayItem, TrayMenuItem, TrayStatus};

use super::icons;
use super::worker::{run_action, spawn_watcher};
use crate::scripting::bridge::{list_value, number_arg, string, string_arg, struct_value};
use crate::scripting::{ShellHost, host};

thread_local! {
    static ITEMS: RefCell<Vec<TrayItem>> = RefCell::default();
    static OPEN_MENU: RefCell<Option<TrayItem>> = RefCell::default();
}

fn icon(item: &TrayItem) -> Image {
    if let Some(pixels) = &item.icon {
        let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
            &pixels.rgba,
            pixels.width,
            pixels.height,
        );
        return Image::from_rgba8(buffer);
    }
    // Some applications ship their icons in a private directory.
    if !item.icon_theme_path.is_empty() {
        for ext in ["svg", "png"] {
            let path = Path::new(&item.icon_theme_path).join(format!("{}.{ext}", item.icon_name));
            if path.is_file() {
                return icons::image_at(&path);
            }
        }
    }
    icons::icon(&item.icon_name, 32)
}

fn publish(host: &ShellHost, items: Vec<TrayItem>) {
    let values = items.iter().map(|item| {
        struct_value([
            ("id", string(&item.service)),
            ("title", string(if item.title.is_empty() { &item.id } else { &item.title })),
            ("tooltip", string(&item.tooltip)),
            ("icon", Value::Image(icon(item))),
            (
                "status",
                string(match item.status {
                    TrayStatus::Active => "active",
                    TrayStatus::Passive => "passive",
                    TrayStatus::NeedsAttention => "attention",
                }),
            ),
            ("has-menu", Value::Bool(!item.menu_path.is_empty())),
            ("item-is-menu", Value::Bool(item.item_is_menu)),
        ])
    });
    host.set_global("Tray", "items", list_value(values.collect::<Vec<_>>()));
    ITEMS.with(|i| *i.borrow_mut() = items);
}

fn publish_menu(host: &ShellHost, owner: &str, menu: &[TrayMenuItem]) {
    host.set_global("Tray", "menu-owner", string(owner));
    host.set_global(
        "Tray",
        "menu",
        list_value(menu.iter().map(|m| {
            struct_value([
                ("id", Value::Number(m.id.into())),
                ("label", string(&m.label)),
                ("enabled", Value::Bool(m.enabled)),
                ("separator", Value::Bool(m.separator)),
                ("toggle-type", string(&m.toggle_type)),
                ("checked", Value::Bool(m.toggle_state == 1)),
                ("depth", Value::Number(m.depth.into())),
                ("has-children", Value::Bool(m.has_children)),
            ])
        })),
    );
}

fn item(id: &str) -> Option<TrayItem> {
    ITEMS.with(|items| items.borrow().iter().find(|i| i.service == id).cloned())
}

/// Registers a callback acting on the item named by the first argument; the optional second
/// argument is a number (the scroll delta).
fn item_action(
    host: &ShellHost,
    tray: &Tray,
    name: &str,
    act: fn(&Tray, &TrayItem, i32) -> zbus::Result<()>,
) {
    let tray = tray.clone();
    host.on_callback(
        "Tray",
        name,
        Rc::new(move |args| {
            if let Some(item) = string_arg(args, 0).and_then(|id| item(&id)) {
                let tray = tray.clone();
                let number = number_arg(args, 1).unwrap_or(0.0) as i32;
                run_action("tray", move || {
                    act(&tray, &item, number).map_err(std::io::Error::other)
                });
            }
            Value::Void
        }),
    );
}

/// Drops cached items and images while Slint is alive.
pub fn shutdown() {
    ITEMS.with(|i| i.borrow_mut().clear());
    OPEN_MENU.with(|m| m.borrow_mut().take());
}

pub fn install(host: &Rc<ShellHost>) {
    let tray = match Tray::connect() {
        Ok(tray) => tray,
        Err(e) => {
            log::info!("tray: unavailable ({e})");
            host.set_global("Tray", "available", Value::Bool(false));
            return;
        }
    };
    host.set_global("Tray", "available", Value::Bool(true));

    let watcher = tray.clone();
    spawn_watcher("tray", move || {
        watcher
            .watch(|items| {
                let items = items.to_vec();
                host::post(move |host| publish(host, items));
            })
            .map_err(std::io::Error::other)
    });

    item_action(host, &tray, "activate", |t, i, _| t.activate(&i.service, 0, 0));
    item_action(host, &tray, "secondary-activate", |t, i, _| {
        t.secondary_activate(&i.service, 0, 0)
    });
    item_action(host, &tray, "scroll", |t, i, delta| t.scroll(&i.service, delta, true));

    let t = tray.clone();
    host.on_callback(
        "Tray",
        "open-menu",
        Rc::new(move |args| {
            let Some(item) = string_arg(args, 0).and_then(|id| item(&id)) else {
                return Value::Void;
            };
            let tray = t.clone();
            if item.menu_path.is_empty() {
                run_action("tray", move || {
                    tray.context_menu(&item.service, 0, 0).map_err(std::io::Error::other)
                });
                return Value::Void;
            }
            OPEN_MENU.with(|m| *m.borrow_mut() = Some(item.clone()));
            std::thread::spawn(move || match tray.menu(&item) {
                Ok(menu) => host::post(move |host| publish_menu(host, &item.service, &menu)),
                Err(e) => log::warn!("tray: cannot load the menu of {}: {e}", item.service),
            });
            Value::Void
        }),
    );
    let weak = Rc::downgrade(host);
    host.on_callback(
        "Tray",
        "close-menu",
        Rc::new(move |_| {
            OPEN_MENU.with(|m| m.borrow_mut().take());
            if let Some(host) = weak.upgrade() {
                publish_menu(&host, "", &[]);
            }
            Value::Void
        }),
    );
    let weak = Rc::downgrade(host);
    host.on_callback(
        "Tray",
        "menu-activate",
        Rc::new(move |args| {
            let (Some(entry), Some(item)) =
                (number_arg(args, 0), OPEN_MENU.with(|m| m.borrow_mut().take()))
            else {
                return Value::Void;
            };
            let tray = tray.clone();
            run_action("tray", move || {
                tray.menu_clicked(&item, entry as i32).map_err(std::io::Error::other)
            });
            if let Some(host) = weak.upgrade() {
                publish_menu(&host, "", &[]);
            }
            Value::Void
        }),
    );
}
