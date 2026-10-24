//! Session locking through `ext-session-lock-v1`.
//!
//! [`lock_session`] asks the compositor to lock; the shell must then show a window with the
//! [`SurfaceRole::Lock`](crate::SurfaceRole::Lock) role on every output. Only
//! [`unlock_session`] ends the lock — if the shell crashes, the compositor keeps the session
//! locked.

use std::rc::Rc;

use crate::core::context;

/// Progress of a lock request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LockEvent {
    /// The compositor confirmed the lock: the screens are covered.
    Locked,
    /// The lock ended without [`unlock_session`] (denied, or another locker took over).
    Finished,
}

/// Requests a session lock. Fails when the compositor lacks `ext-session-lock-v1` or a lock is
/// already active.
pub fn lock_session() -> Result<(), String> {
    let ctx = context::current().ok_or("the SlintLu platform is not running")?;
    let mut lock = ctx.session_lock.borrow_mut();
    if lock.is_some() {
        return Err("the session is already locked".into());
    }
    let state = ctx
        .session_lock_state
        .as_ref()
        .ok_or("the compositor does not support ext-session-lock-v1")?;
    *lock = Some(state.lock(&ctx.qh).map_err(|e| format!("cannot lock the session: {e}"))?);
    Ok(())
}

/// Ends the session lock. Destroy the lock windows first.
pub fn unlock_session() {
    if let Some(ctx) = context::current()
        && let Some(lock) = ctx.session_lock.borrow_mut().take()
    {
        lock.unlock();
        let _ = ctx.conn.flush();
    }
}

/// Whether a lock is requested or active.
pub fn session_locked() -> bool {
    context::current().is_some_and(|ctx| ctx.session_lock.borrow().is_some())
}

/// Registers a callback for [`LockEvent`]s.
pub fn on_lock_event(callback: impl Fn(LockEvent) + 'static) {
    if let Some(ctx) = context::current() {
        ctx.lock_listeners.borrow_mut().push(Rc::new(callback));
    }
}
