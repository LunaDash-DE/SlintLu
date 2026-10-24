//! The `Polkit` global: SlintLu as the session's polkit authentication agent.

use std::cell::RefCell;
use std::rc::Rc;

use slint_interpreter::Value;
use slintlu::security::polkit::{self, PolkitEvent, PolkitHandle, Subject};

use crate::scripting::bridge::{string, string_arg};
use crate::scripting::{ShellHost, host};

thread_local! {
    static HANDLE: RefCell<Option<PolkitHandle>> = const { RefCell::new(None) };
    static CURRENT: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn on_event(host: &ShellHost, event: PolkitEvent) {
    let set = |name: &str, value: Value| host.set_global("Polkit", name, value);
    match event {
        PolkitEvent::Begin { cookie, action_id, message, icon_name, user } => {
            if CURRENT.with(|c| c.borrow().is_some()) {
                // One dialog at a time; polkit asks again later.
                HANDLE.with(|h| h.borrow().as_ref().map(|h| h.cancel(&cookie)));
                return;
            }
            CURRENT.with(|c| *c.borrow_mut() = Some(cookie));
            set("message", string(&message));
            set("action-id", string(&action_id));
            set("icon-name", string(&icon_name));
            set("user", string(&user));
            set("prompt", string(""));
            set("error", string(""));
            set("authenticating", Value::Bool(false));
            set("active", Value::Bool(true));
        }
        PolkitEvent::Prompt { cookie, prompt, echo } if is_current(&cookie) => {
            set("prompt", string(&prompt));
            set("echo", Value::Bool(echo));
            set("authenticating", Value::Bool(false));
        }
        PolkitEvent::Info { cookie, text, error } if is_current(&cookie) => {
            if error {
                set("error", string(&text));
            }
            set("authenticating", Value::Bool(false));
        }
        PolkitEvent::Finished { cookie, .. } if is_current(&cookie) => {
            CURRENT.with(|c| c.borrow_mut().take());
            set("active", Value::Bool(false));
            set("authenticating", Value::Bool(false));
        }
        _ => {}
    }
}

fn is_current(cookie: &str) -> bool {
    CURRENT.with(|c| c.borrow().as_deref() == Some(cookie))
}

/// Unregisters while the connection is still up.
pub fn shutdown() {
    if let Some(handle) = HANDLE.with(|h| h.borrow_mut().take()) {
        handle.unregister();
    }
}

pub fn install(host: &Rc<ShellHost>) {
    let Some(subject) = Subject::current_session() else {
        log::info!("polkit: XDG_SESSION_ID is not set, not registering an agent");
        host.set_global("Polkit", "available", Value::Bool(false));
        return;
    };
    let handle = polkit::register(subject, |event| host::post(move |host| on_event(host, event)));
    match handle {
        Ok(handle) => {
            HANDLE.with(|h| *h.borrow_mut() = Some(handle));
            host.set_global("Polkit", "available", Value::Bool(true));
        }
        Err(e) => {
            log::info!("polkit: not the authentication agent ({e}); another agent is running");
            host.set_global("Polkit", "available", Value::Bool(false));
            return;
        }
    }

    let weak = Rc::downgrade(host);
    host.on_callback(
        "Polkit",
        "respond",
        Rc::new(move |args| {
            let (Some(text), Some(cookie)) =
                (string_arg(args, 0), CURRENT.with(|c| c.borrow().clone()))
            else {
                return Value::Void;
            };
            if let Some(host) = weak.upgrade() {
                host.set_global("Polkit", "authenticating", Value::Bool(true));
                host.set_global("Polkit", "error", string(""));
            }
            HANDLE.with(|h| h.borrow().as_ref().map(|h| h.respond(&cookie, text)));
            Value::Void
        }),
    );
    host.on_callback(
        "Polkit",
        "cancel",
        Rc::new(|_| {
            if let Some(cookie) = CURRENT.with(|c| c.borrow().clone()) {
                HANDLE.with(|h| h.borrow().as_ref().map(|h| h.cancel(&cookie)));
            }
            Value::Void
        }),
    );
}
