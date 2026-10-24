//! `xdg_toplevel` windows for regular application-style windows (settings, dialogs …).

use smithay_client_toolkit::compositor::Surface;
use smithay_client_toolkit::reexports::client::{Connection, QueueHandle};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shell::xdg::window::{
    Window, WindowConfigure, WindowDecorations, WindowHandler,
};

use crate::core::context::Context;
use crate::core::state::State;

/// Creates the toplevel. Returns the surface back if xdg-shell is unavailable.
pub(crate) fn create(
    ctx: &Context,
    surface: Surface,
    app_id: &str,
    title: &str,
    min_size: Option<(u32, u32)>,
) -> Result<Window, Surface> {
    let Some(xdg_shell) = &ctx.xdg_shell else { return Err(surface) };
    let window = xdg_shell.create_window(surface, WindowDecorations::RequestServer, &ctx.qh);
    window.set_app_id(app_id);
    window.set_title(title);
    window.set_min_size(min_size);
    Ok(window)
}

impl WindowHandler for State {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, window: &Window) {
        if let Some(window) = self.ctx.find_window(window.wl_surface()) {
            window.request_close();
        }
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        window: &Window,
        configure: WindowConfigure,
        _: u32,
    ) {
        if let Some(window) = self.ctx.find_window(window.wl_surface()) {
            let (w, h) = configure.new_size;
            window.configure(w.map_or(0, |w| w.get()), h.map_or(0, |h| h.get()));
        }
    }
}
