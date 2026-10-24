//! The `Session` global: locking with a shell-defined lock screen and unlocking with PAM.

use std::cell::Cell;
use std::rc::Rc;

use slint_interpreter::Value;
use slintlu::dbus::power::{Power, SessionEvent};
use slintlu::security::{self, LockEvent};

use super::worker::spawn_watcher;
use crate::scripting::bridge::{string, string_arg};
use crate::scripting::{ShellHost, host};

thread_local! {
    static AUTHENTICATING: Cell<bool> = const { Cell::new(false) };
}

fn set_locked_hint(locked: bool) {
    std::thread::spawn(move || {
        if let Ok(power) = Power::connect()
            && let Err(e) = power.set_locked_hint(locked)
        {
            log::debug!("cannot set the logind locked hint: {e}");
        }
    });
}

/// Locks the session with the shell's lock screen.
pub fn lock(host: &ShellHost) {
    if security::session_locked() {
        return;
    }
    if !host.has_lock_screen() {
        // Locking without a lock screen would leave the user stuck on a blank screen.
        return log::error!("cannot lock: the shell has no component inheriting LockWindow");
    }
    if let Err(e) = security::lock_session() {
        return log::error!("{e}");
    }
    host.set_global("Session", "message", string(""));
    host.show_lock_screens();
}

fn unlock(host: &ShellHost) {
    host.hide_lock_screens();
    security::unlock_session();
    host.set_global("Session", "locked", Value::Bool(false));
    host.set_global("Session", "authenticating", Value::Bool(false));
    host.set_global("Session", "message", string(""));
    set_locked_hint(false);
}

fn authenticate(host: &ShellHost, password: String) {
    if !security::session_locked() || AUTHENTICATING.replace(true) {
        return;
    }
    host.set_global("Session", "authenticating", Value::Bool(true));
    host.set_global("Session", "message", string(""));
    std::thread::spawn(move || {
        let result = security::authenticate(
            &security::default_service(),
            &security::current_user(),
            &password,
        );
        drop(password);
        host::post(move |host| {
            AUTHENTICATING.set(false);
            host.set_global("Session", "authenticating", Value::Bool(false));
            match result {
                Ok(()) => unlock(host),
                Err(e) => {
                    log::info!("unlock failed: {e}");
                    host.set_global("Session", "message", string(e.to_string()));
                }
            }
        });
    });
}

pub fn install(host: &Rc<ShellHost>) {
    host.set_global("Session", "user", string(security::current_user()));

    let weak = Rc::downgrade(host);
    security::on_lock_event(move |event| {
        let Some(host) = weak.upgrade() else { return };
        match event {
            LockEvent::Locked => {
                host.set_global("Session", "locked", Value::Bool(true));
                set_locked_hint(true);
            }
            LockEvent::Finished => {
                log::warn!("the compositor ended the session lock");
                host.hide_lock_screens();
                host.set_global("Session", "locked", Value::Bool(false));
            }
        }
    });

    host.on_callback(
        "Session",
        "lock",
        Rc::new(|_| {
            // Not from inside a window's own callback: windows are created while locking.
            host::post(|host| lock(host));
            Value::Void
        }),
    );
    host.on_callback(
        "Session",
        "authenticate",
        Rc::new(|args| {
            let password = string_arg(args, 0).unwrap_or_default();
            host::post(move |host| authenticate(host, password));
            Value::Void
        }),
    );

    // logind: `loginctl lock-session`, idle daemons and suspend.
    if let Ok(power) = Power::connect() {
        spawn_watcher("session", move || {
            power
                .watch(|event| match event {
                    SessionEvent::Lock => host::post(|host| lock(host)),
                    SessionEvent::Unlock => host::post(|host| {
                        if security::session_locked() {
                            unlock(host);
                        }
                    }),
                    SessionEvent::Sleep(true) => host::post(|host| {
                        if matches!(
                            host.global_property("Session", "lock-before-sleep"),
                            Some(Value::Bool(true)) | None
                        ) {
                            lock(host);
                        }
                    }),
                    SessionEvent::Sleep(false) => {}
                })
                .map_err(std::io::Error::other)
        });
    }
}
