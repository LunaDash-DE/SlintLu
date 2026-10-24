//! The `Notifications` global: SlintLu as the session's notification daemon.

use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::time::Duration;

use slint::{Timer, TimerMode};
use slint_interpreter::Value;
use slintlu::dbus::notifications::{
    self, CloseReason, Notification, NotificationEvent, NotificationHandle, Urgency,
};

use crate::scripting::bridge::{list_value, number_arg, string, string_arg, struct_value};
use crate::scripting::{ShellHost, host};

const FALLBACK_TIMEOUT: Duration = Duration::from_secs(5);

struct Entry {
    notification: Notification,
    time: String,
    popup: bool,
    /// Expires the pop-up; dropping it cancels the timer.
    timer: Option<Timer>,
}

struct Store {
    handle: NotificationHandle,
    /// Newest first.
    entries: Vec<Entry>,
    host: Weak<ShellHost>,
}

fn urgency(u: Urgency) -> &'static str {
    match u {
        Urgency::Low => "low",
        Urgency::Normal => "normal",
        Urgency::Critical => "critical",
    }
}

fn value(entry: &Entry) -> Value {
    let n = &entry.notification;
    struct_value([
        ("id", Value::Number(n.id.into())),
        ("app-name", string(&n.app_name)),
        ("app-icon", string(&n.app_icon)),
        ("image", string(&n.image)),
        ("summary", string(&n.summary)),
        ("body", string(&n.body)),
        ("urgency", string(urgency(n.urgency))),
        (
            "actions",
            list_value(n.actions.iter().map(|(key, label)| {
                struct_value([("key", string(key)), ("label", string(label))])
            })),
        ),
        ("time", string(&entry.time)),
    ])
}

impl Store {
    fn publish(&self) {
        let Some(host) = self.host.upgrade() else { return };
        host.set_global(
            "Notifications",
            "popups",
            list_value(self.entries.iter().filter(|e| e.popup).map(value)),
        );
        host.set_global("Notifications", "history", list_value(self.entries.iter().map(value)));
        host.set_global("Notifications", "count", Value::Number(self.entries.len() as f64));
    }

    fn remove(&mut self, id: u32) -> Option<Entry> {
        let index = self.entries.iter().position(|e| e.notification.id == id)?;
        Some(self.entries.remove(index))
    }
}

fn setting(host: &ShellHost, name: &str) -> Option<Value> {
    host.global_property("Notifications", name)
}

fn on_notify(store: &Rc<RefCell<Store>>, notification: Notification) {
    let Some(host) = store.borrow().host.upgrade() else { return };
    let do_not_disturb = matches!(setting(&host, "do-not-disturb"), Some(Value::Bool(true)));
    let popup = !do_not_disturb || notification.urgency == Urgency::Critical;
    let timeout = match notification.timeout {
        Some(0) => None,
        Some(ms) => Some(Duration::from_millis(ms.into())),
        // Critical notifications stay until dismissed, as the specification suggests.
        None if notification.urgency == Urgency::Critical => None,
        None => Some(match setting(&host, "default-timeout") {
            Some(Value::Number(ms)) if ms > 0.0 => Duration::from_millis(ms as u64),
            _ => FALLBACK_TIMEOUT,
        }),
    };

    let id = notification.id;
    let timer = timeout.filter(|_| popup).map(|timeout| {
        let timer = Timer::default();
        let weak = Rc::downgrade(store);
        timer.start(TimerMode::SingleShot, timeout, move || {
            if let Some(store) = weak.upgrade() {
                expire(&store, id);
            }
        });
        timer
    });
    let entry = Entry {
        time: slintlu::time::read(slintlu::time::now(), "%H:%M", "").time,
        notification,
        popup,
        timer,
    };
    let mut s = store.borrow_mut();
    s.remove(id);
    s.entries.insert(0, entry);
    s.publish();
}

fn expire(store: &Rc<RefCell<Store>>, id: u32) {
    let mut s = store.borrow_mut();
    let Some(entry) = s.entries.iter_mut().find(|e| e.notification.id == id) else { return };
    entry.popup = false;
    entry.timer = None;
    if entry.notification.transient {
        s.remove(id);
    }
    s.handle.close(id, CloseReason::Expired);
    s.publish();
}

fn dismiss(store: &Rc<RefCell<Store>>, id: u32, reason: CloseReason) {
    let mut s = store.borrow_mut();
    if s.remove(id).is_some() {
        s.handle.close(id, reason);
        s.publish();
    }
}

pub fn install(host: &Rc<ShellHost>) {
    let replace = std::env::var_os("SLINTLU_REPLACE_NOTIFICATIONS").is_some();
    let handle = notifications::serve(replace, |event| {
        host::post(move |_| {
            with_store(|store| match event {
                NotificationEvent::Notify(n) => on_notify(store, n),
                NotificationEvent::Closed(id) => dismiss(store, id, CloseReason::Closed),
            })
        });
    });
    let handle = match handle {
        Ok(handle) => handle,
        Err(e) => {
            log::info!(
                "notifications: not serving ({e}); another daemon owns the name \
                 (set SLINTLU_REPLACE_NOTIFICATIONS=1 to take over)"
            );
            host.set_global("Notifications", "available", Value::Bool(false));
            return;
        }
    };
    host.set_global("Notifications", "available", Value::Bool(true));
    let store =
        Rc::new(RefCell::new(Store { handle, entries: Vec::new(), host: Rc::downgrade(host) }));
    STORE.with(|s| *s.borrow_mut() = Some(store.clone()));
    store.borrow().publish();

    let s = store.clone();
    host.on_callback(
        "Notifications",
        "dismiss",
        Rc::new(move |args| {
            if let Some(id) = number_arg(args, 0) {
                dismiss(&s, id as u32, CloseReason::Dismissed);
            }
            Value::Void
        }),
    );
    let s = store.clone();
    host.on_callback(
        "Notifications",
        "dismiss-all",
        Rc::new(move |_| {
            let ids: Vec<u32> = s.borrow().entries.iter().map(|e| e.notification.id).collect();
            for id in ids {
                dismiss(&s, id, CloseReason::Dismissed);
            }
            Value::Void
        }),
    );
    let s = store;
    host.on_callback(
        "Notifications",
        "invoke",
        Rc::new(move |args| {
            if let (Some(id), Some(key)) = (number_arg(args, 0), string_arg(args, 1)) {
                let id = id as u32;
                s.borrow().handle.invoke_action(id, &key);
                dismiss(&s, id, CloseReason::Dismissed);
            }
            Value::Void
        }),
    );
}

thread_local! {
    static STORE: RefCell<Option<Rc<RefCell<Store>>>> = const { RefCell::new(None) };
}

/// Drops pending expiry timers while Slint is still alive.
pub fn shutdown() {
    STORE.with(|s| s.borrow_mut().take());
}

fn with_store(f: impl FnOnce(&Rc<RefCell<Store>>)) {
    if let Some(store) = STORE.with(|s| s.borrow().clone()) {
        f(&store);
    }
}
