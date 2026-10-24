//! The system tray: a StatusNotifierItem host (and watcher, when the session has none) with
//! `com.canonical.dbusmenu` menus.

mod item;
mod menu;
mod watcher;

use std::sync::{Arc, Mutex};

use futures_util::StreamExt;
use zbus::fdo::DBusProxy;
use zbus::names::BusName;
use zbus::{Connection, MatchRule, MessageStream};

pub use item::{TrayIcon, TrayItem, TrayStatus};
pub use menu::TrayMenuItem;

const WATCHER: &str = "org.kde.StatusNotifierWatcher";
const WATCHER_PATH: &str = "/StatusNotifierWatcher";

/// Connection to the tray.
#[derive(Clone)]
pub struct Tray {
    connection: Connection,
    /// Items registered with our own watcher, when we provide it.
    own_items: Option<Arc<Mutex<Vec<String>>>>,
}

impl Tray {
    /// Connects to the session bus, becoming the watcher if nobody else is, and registers as a
    /// host.
    pub fn connect() -> zbus::Result<Self> {
        zbus::block_on(async {
            let connection = Connection::session().await?;
            let own_items = watcher::serve_if_missing(&connection).await?;
            let host = format!("org.kde.StatusNotifierHost-{}", std::process::id());
            connection.request_name(host.as_str()).await?;
            watcher::WatcherProxy::new(&connection)
                .await?
                .register_status_notifier_host(&host)
                .await?;
            log::info!(
                "tray: {} the StatusNotifierWatcher",
                if own_items.is_some() { "providing" } else { "using an existing" }
            );
            Ok(Self { connection, own_items })
        })
    }

    async fn services(&self) -> zbus::Result<Vec<String>> {
        if let Some(items) = &self.own_items {
            return Ok(items.lock().unwrap_or_else(|e| e.into_inner()).clone());
        }
        watcher::WatcherProxy::new(&self.connection).await?.registered_status_notifier_items().await
    }

    async fn snapshot(&self) -> zbus::Result<Vec<TrayItem>> {
        let dbus = DBusProxy::new(&self.connection).await?;
        let mut items = Vec::new();
        for service in self.services().await? {
            let (bus, path) = item::split_service(&service);
            // Skip items whose application is gone but are still listed.
            let Ok(name) = BusName::try_from(bus.as_str()) else { continue };
            if !dbus.name_has_owner(name).await.unwrap_or(false) {
                continue;
            }
            match item::read(&self.connection, &service, &bus, &path).await {
                Ok(item) => items.push(item),
                Err(e) => log::debug!("tray: cannot read {service}: {e}"),
            }
        }
        Ok(items)
    }

    pub fn items(&self) -> zbus::Result<Vec<TrayItem>> {
        zbus::block_on(self.snapshot())
    }

    /// Blocks, calling `on_items` initially and whenever items appear, vanish or change.
    pub fn watch(&self, mut on_items: impl FnMut(&[TrayItem])) -> zbus::Result<()> {
        zbus::block_on(async {
            let item_signals = MatchRule::builder()
                .msg_type(zbus::message::Type::Signal)
                .interface("org.kde.StatusNotifierItem")
                .build()?;
            let watcher_signals = MatchRule::builder()
                .msg_type(zbus::message::Type::Signal)
                .interface(WATCHER)
                .build()?;
            let owners = MatchRule::builder()
                .msg_type(zbus::message::Type::Signal)
                .sender("org.freedesktop.DBus")
                .member("NameOwnerChanged")
                .build()?;
            let mut item_signals =
                MessageStream::for_match_rule(item_signals, &self.connection, None).await?;
            let mut watcher_signals =
                MessageStream::for_match_rule(watcher_signals, &self.connection, None).await?;
            let mut owners = MessageStream::for_match_rule(owners, &self.connection, None).await?;
            let mut last = self.snapshot().await?;
            on_items(&last);
            loop {
                futures_util::select! {
                    m = item_signals.next() => if m.is_none() { break },
                    m = watcher_signals.next() => if m.is_none() { break },
                    m = owners.next() => if m.is_none() { break },
                }
                let items = self.snapshot().await?;
                if items != last {
                    on_items(&items);
                    last = items;
                }
            }
            Err(zbus::Error::Failure("the session bus closed".into()))
        })
    }

    /// Left click. `x`/`y` are screen coordinates for the application's own popups.
    pub fn activate(&self, service: &str, x: i32, y: i32) -> zbus::Result<()> {
        zbus::block_on(async { item::proxy(&self.connection, service).await?.activate(x, y).await })
    }

    /// Middle click.
    pub fn secondary_activate(&self, service: &str, x: i32, y: i32) -> zbus::Result<()> {
        zbus::block_on(async {
            item::proxy(&self.connection, service).await?.secondary_activate(x, y).await
        })
    }

    /// Asks the application to show its own context menu (used when it has no dbusmenu).
    pub fn context_menu(&self, service: &str, x: i32, y: i32) -> zbus::Result<()> {
        zbus::block_on(async {
            item::proxy(&self.connection, service).await?.context_menu(x, y).await
        })
    }

    pub fn scroll(&self, service: &str, delta: i32, vertical: bool) -> zbus::Result<()> {
        zbus::block_on(async {
            let orientation = if vertical { "vertical" } else { "horizontal" };
            item::proxy(&self.connection, service).await?.scroll(delta, orientation).await
        })
    }

    /// The item's menu, flattened depth-first (see [`TrayMenuItem::depth`]).
    pub fn menu(&self, item: &TrayItem) -> zbus::Result<Vec<TrayMenuItem>> {
        zbus::block_on(menu::fetch(&self.connection, &item.bus_name, &item.menu_path))
    }

    /// Runs a menu entry.
    pub fn menu_clicked(&self, item: &TrayItem, id: i32) -> zbus::Result<()> {
        zbus::block_on(menu::clicked(&self.connection, &item.bus_name, &item.menu_path, id))
    }
}
