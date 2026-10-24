//! An `org.freedesktop.Notifications` server (notification daemon).
//!
//! Applications call `Notify`; the shell decides how to show it and reports back dismissals
//! and invoked actions through a [`NotificationHandle`].

use std::collections::HashMap;
use std::sync::mpsc;

use zbus::fdo::{RequestNameFlags, RequestNameReply};
use zbus::object_server::SignalEmitter;
use zbus::{OwnedVariant, Variant, connection, interface};

use super::markup;

const PATH: &str = "/org/freedesktop/Notifications";
const NAME: &str = "org.freedesktop.Notifications";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Urgency {
    Low,
    #[default]
    Normal,
    Critical,
}

/// One notification as sent by an application.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Notification {
    pub id: u32,
    pub app_name: String,
    /// Icon name or `file://` / absolute path.
    pub app_icon: String,
    /// Path of an image to show (from the `image-path` hint), if any.
    pub image: String,
    pub summary: String,
    /// Plain text (markup removed).
    pub body: String,
    /// `(key, label)` pairs; the key `default` is the action for clicking the notification.
    pub actions: Vec<(String, String)>,
    pub urgency: Urgency,
    /// Milliseconds; `None` means the server decides, `Some(0)` never expires.
    pub timeout: Option<u32>,
    pub desktop_entry: String,
    /// Should not be kept in a history once shown.
    pub transient: bool,
}

/// Why a notification disappeared, as defined by the specification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseReason {
    Expired = 1,
    Dismissed = 2,
    Closed = 3,
    Undefined = 4,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NotificationEvent {
    /// A new notification, or a replacement for one with the same id.
    Notify(Notification),
    /// The application withdrew a notification.
    Closed(u32),
}

enum Command {
    Close(u32, CloseReason),
    Action(u32, String),
}

/// Lets the shell report what the user did with a notification.
#[derive(Clone)]
pub struct NotificationHandle {
    commands: mpsc::Sender<Command>,
    wake: async_channel_like::Waker,
}

impl NotificationHandle {
    pub fn close(&self, id: u32, reason: CloseReason) {
        let _ = self.commands.send(Command::Close(id, reason));
        self.wake.wake();
    }

    pub fn invoke_action(&self, id: u32, key: &str) {
        let _ = self.commands.send(Command::Action(id, key.to_owned()));
        self.wake.wake();
    }
}

/// A tiny wake-up primitive between the shell's thread and the D-Bus thread.
mod async_channel_like {
    use std::sync::{Arc, Condvar, Mutex};

    #[derive(Clone, Default)]
    pub struct Waker(Arc<(Mutex<bool>, Condvar)>);

    impl Waker {
        pub fn wake(&self) {
            let (flag, cv) = &*self.0;
            *flag.lock().unwrap_or_else(|e| e.into_inner()) = true;
            cv.notify_all();
        }

        /// Blocks until woken.
        pub fn wait(&self) {
            let (flag, cv) = &*self.0;
            let mut woken = flag.lock().unwrap_or_else(|e| e.into_inner());
            while !*woken {
                woken = cv.wait(woken).unwrap_or_else(|e| e.into_inner());
            }
            *woken = false;
        }
    }
}

type Sink = Box<dyn Fn(NotificationEvent) + Send + Sync>;

struct Server {
    next_id: u32,
    sink: Sink,
}

fn hint_str(hints: &HashMap<String, OwnedVariant>, key: &str) -> String {
    hints.get(key).and_then(|v| String::try_from(&**v).ok()).unwrap_or_default()
}

fn hint_bool(hints: &HashMap<String, OwnedVariant>, key: &str) -> bool {
    hints.get(key).and_then(|v| bool::try_from(&**v).ok()).unwrap_or(false)
}

fn urgency(hints: &HashMap<String, OwnedVariant>) -> Urgency {
    let value = hints.get("urgency").and_then(|v| match &**v {
        Variant::U8(u) => Some(u32::from(*u)),
        Variant::U32(u) => Some(*u),
        Variant::I32(i) => u32::try_from(*i).ok(),
        _ => None,
    });
    match value {
        Some(0) => Urgency::Low,
        Some(2) => Urgency::Critical,
        _ => Urgency::Normal,
    }
}

fn pairs(actions: Vec<String>) -> Vec<(String, String)> {
    actions.chunks(2).filter_map(|c| Some((c.first()?.clone(), c.get(1)?.clone()))).collect()
}

#[interface(name = "org.freedesktop.Notifications")]
impl Server {
    fn get_capabilities(&self) -> Vec<&'static str> {
        vec!["body", "actions", "icon-static", "persistence"]
    }

    #[allow(clippy::too_many_arguments)]
    fn notify(
        &mut self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedVariant>,
        expire_timeout: i32,
    ) -> u32 {
        let id = if replaces_id != 0 {
            replaces_id
        } else {
            self.next_id = self.next_id.wrapping_add(1).max(1);
            self.next_id
        };
        let image = [hint_str(&hints, "image-path"), hint_str(&hints, "image_path")]
            .into_iter()
            .find(|s| !s.is_empty())
            .unwrap_or_default();
        (self.sink)(NotificationEvent::Notify(Notification {
            id,
            app_name,
            app_icon,
            image,
            summary: markup::strip(&summary),
            body: markup::strip(&body),
            actions: pairs(actions),
            urgency: urgency(&hints),
            timeout: u32::try_from(expire_timeout).ok(),
            desktop_entry: hint_str(&hints, "desktop-entry"),
            transient: hint_bool(&hints, "transient"),
        }));
        id
    }

    fn close_notification(&self, id: u32) {
        (self.sink)(NotificationEvent::Closed(id));
    }

    fn get_server_information(&self) -> (&'static str, &'static str, &'static str, &'static str) {
        ("slintlu", "LunaDash", env!("CARGO_PKG_VERSION"), "1.2")
    }

    #[zbus(signal)]
    async fn notification_closed(
        emitter: &SignalEmitter<'_>,
        id: u32,
        reason: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn action_invoked(
        emitter: &SignalEmitter<'_>,
        id: u32,
        action_key: &str,
    ) -> zbus::Result<()>;
}

/// Claims `org.freedesktop.Notifications` on the session bus and serves it from a background
/// thread.
///
/// When another daemon (mako, dunst, a different shell …) owns the name this fails, unless
/// `replace` is set and that daemon allows being replaced. SlintLu itself always allows being
/// replaced.
pub fn serve(
    replace: bool,
    sink: impl Fn(NotificationEvent) + Send + Sync + 'static,
) -> zbus::Result<NotificationHandle> {
    let connection = zbus::block_on(async {
        let connection = connection::Builder::session()
            .serve_at(PATH, Server { next_id: 0, sink: Box::new(sink) })
            .build()
            .await?;
        let mut flags = RequestNameFlags::DO_NOT_QUEUE | RequestNameFlags::ALLOW_REPLACEMENT;
        if replace {
            flags |= RequestNameFlags::REPLACE_EXISTING;
        }
        match connection.request_name_with_flags(NAME, flags).await? {
            RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner => Ok(connection),
            _ => Err(zbus::Error::NameTaken),
        }
    })?;
    let (commands, receiver) = mpsc::channel::<Command>();
    let wake = async_channel_like::Waker::default();
    let waker = wake.clone();

    std::thread::Builder::new()
        .name("slintlu-notifications".into())
        .spawn(move || {
            let emitter = match SignalEmitter::new(&connection, PATH) {
                Ok(emitter) => emitter,
                Err(e) => return log::error!("notifications: {e}"),
            };
            loop {
                waker.wait();
                for command in receiver.try_iter() {
                    let sent = zbus::block_on(async {
                        match command {
                            Command::Close(id, reason) => {
                                Server::notification_closed(&emitter, id, reason as u32).await
                            }
                            Command::Action(id, key) => {
                                Server::action_invoked(&emitter, id, &key).await
                            }
                        }
                    });
                    if let Err(e) = sent {
                        log::warn!("notifications: cannot emit signal: {e}");
                    }
                }
            }
        })
        .map_err(|e| zbus::Error::Failure(e.to_string()))?;
    Ok(NotificationHandle { commands, wake })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actions_come_in_pairs() {
        let actions =
            vec!["default".into(), "Open".into(), "reply".into(), "Reply".into(), "odd".into()];
        assert_eq!(
            pairs(actions),
            vec![("default".into(), "Open".into()), ("reply".into(), "Reply".into())]
        );
    }

    #[test]
    fn urgency_hint() {
        let mut hints = HashMap::new();
        assert_eq!(urgency(&hints), Urgency::Normal);
        hints.insert("urgency".into(), OwnedVariant::from(2u8));
        assert_eq!(urgency(&hints), Urgency::Critical);
        hints.insert("urgency".into(), OwnedVariant::from(0u8));
        assert_eq!(urgency(&hints), Urgency::Low);
    }
}
