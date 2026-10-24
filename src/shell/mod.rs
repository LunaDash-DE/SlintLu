//! Surface roles: `wlr-layer-shell` surfaces for shell components and `xdg-shell` toplevels for
//! ordinary windows.

pub(crate) mod config;
pub(crate) mod layer;
pub(crate) mod lock;
pub(crate) mod xdg;

use smithay_client_toolkit::compositor::Surface;
use smithay_client_toolkit::session_lock::SessionLockSurface;
use smithay_client_toolkit::shell::wlr_layer::LayerSurface;
use smithay_client_toolkit::shell::xdg::window::Window as XdgWindow;

/// The protocol object currently giving the window's `wl_surface` its role.
pub(crate) enum RoleObject {
    /// No role yet. The surface is kept here until the window is first shown.
    Unassigned(Option<Surface>),
    Layer(LayerSurface),
    Toplevel(XdgWindow),
    /// Held only to keep the lock surface alive; dropping it destroys the surface.
    Lock(#[allow(dead_code)] SessionLockSurface),
}
