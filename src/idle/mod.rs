//! Idle detection (`ext-idle-notify-v1`) and idle inhibition (`zwp_idle_inhibit_v1`).

mod protocol;

use std::rc::Rc;
use std::time::Duration;

use crate::core::context;
pub(crate) use protocol::IdleState;

/// Keeps an idle watch alive; dropping it stops the notifications.
pub struct IdleWatch {
    id: u32,
}

impl Drop for IdleWatch {
    fn drop(&mut self) {
        if let Some(ctx) = context::current() {
            ctx.idle.remove_watch(self.id);
        }
    }
}

/// Calls `callback(true)` once the user has been idle for `timeout`, and `callback(false)` when
/// they come back. `None` when the compositor lacks `ext-idle-notify-v1`.
pub fn watch_idle(timeout: Duration, callback: impl Fn(bool) + 'static) -> Option<IdleWatch> {
    let ctx = context::current()?;
    let id = ctx.idle.add_watch(&ctx.qh, timeout, Rc::new(callback))?;
    let _ = ctx.conn.flush();
    Some(IdleWatch { id })
}

/// Prevents (or allows again) the screen from blanking and idle actions while a shell window is
/// visible — e.g. a "caffeine" toggle. Returns whether inhibition is active.
pub fn set_idle_inhibited(inhibited: bool) -> bool {
    let Some(ctx) = context::current() else { return false };
    if !inhibited {
        ctx.idle.uninhibit();
        return false;
    }
    // Inhibitors attach to a surface and only work while it is visible: use a shown window.
    let Some(window) = ctx.live_windows().into_iter().find(|w| w.is_shown()) else {
        log::warn!("cannot inhibit idle: no shell window is visible");
        return false;
    };
    let active = ctx.idle.inhibit(window.wl_surface(), &ctx.qh);
    let _ = ctx.conn.flush();
    active
}
