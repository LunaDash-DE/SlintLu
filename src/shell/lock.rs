//! `ext_session_lock_surface_v1`: lock screen surfaces.

use smithay_client_toolkit::compositor::Surface;
use smithay_client_toolkit::reexports::client::{Connection, QueueHandle};
use smithay_client_toolkit::session_lock::{
    SessionLock, SessionLockHandler, SessionLockSurface, SessionLockSurfaceConfigure,
};

use crate::core::context::Context;
use crate::core::state::State;
use crate::security::LockEvent;

/// Creates the lock surface for `output`. Returns the surface back when no lock is active.
pub(crate) fn create(
    ctx: &Context,
    surface: Surface,
    output: u32,
) -> Result<SessionLockSurface, Surface> {
    let lock = ctx.session_lock.borrow();
    let (Some(lock), Some(output)) = (lock.as_ref(), crate::output::wl_output_by_id(ctx, output))
    else {
        return Err(surface);
    };
    Ok(lock.create_lock_surface(surface, &output, &ctx.qh))
}

fn notify(state: &State, event: LockEvent) {
    let listeners = state.ctx.lock_listeners.borrow().clone();
    for listener in listeners {
        listener(event);
    }
}

impl SessionLockHandler for State {
    fn locked(&mut self, _: &Connection, _: &QueueHandle<Self>, _: SessionLock) {
        notify(self, LockEvent::Locked);
    }

    fn finished(&mut self, _: &Connection, _: &QueueHandle<Self>, _: SessionLock) {
        self.ctx.session_lock.borrow_mut().take();
        notify(self, LockEvent::Finished);
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        surface: SessionLockSurface,
        configure: SessionLockSurfaceConfigure,
        _: u32,
    ) {
        if let Some(window) = self.ctx.find_window(surface.wl_surface()) {
            window.configure(configure.new_size.0, configure.new_size.1);
        }
    }
}
