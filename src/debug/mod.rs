//! Diagnostics: what the compositor and the machine support, for `slintlu info` and bug
//! reports.

use crate::core::context;
use crate::render::RendererKind;

/// Wayland protocols SlintLu uses, with what they enable.
pub const PROTOCOLS: &[(&str, &str)] = &[
    ("zwlr_layer_shell_v1", "panels, bars, overlays"),
    ("xdg_wm_base", "normal windows"),
    ("ext_session_lock_manager_v1", "lock screen"),
    ("ext_idle_notifier_v1", "idle detection"),
    ("zwp_idle_inhibit_manager_v1", "idle inhibition"),
    ("wp_fractional_scale_manager_v1", "fractional scaling"),
    ("wp_viewporter", "fractional scaling"),
    ("wp_cursor_shape_manager_v1", "cursor shapes"),
    ("wl_data_device_manager", "clipboard"),
    ("zwp_primary_selection_device_manager_v1", "middle-click paste"),
    ("zwlr_data_control_manager_v1", "clipboard history"),
];

/// Whether each of [`PROTOCOLS`] is offered by the compositor. Empty before [`crate::init`].
pub fn protocol_support() -> Vec<(&'static str, &'static str, bool)> {
    let Some(ctx) = context::current() else { return Vec::new() };
    PROTOCOLS
        .iter()
        .map(|(name, purpose)| (*name, *purpose, ctx.globals.iter().any(|g| g == name)))
        .collect()
}

/// Tries each GPU renderer without opening a window: `Ok(description)` or `Err(reason)`.
pub fn probe_renderers() -> Vec<(RendererKind, Result<String, String>)> {
    let mut results = vec![(RendererKind::Software, Ok("always available".to_owned()))];
    #[cfg(feature = "opengl")]
    results.push((RendererKind::OpenGL, crate::render::egl::probe()));
    #[cfg(not(feature = "opengl"))]
    results.push((RendererKind::OpenGL, Err("not compiled in".to_owned())));
    #[cfg(feature = "vulkan")]
    results.push((RendererKind::Vulkan, crate::render::gpu::probe()));
    #[cfg(not(feature = "vulkan"))]
    results.push((RendererKind::Vulkan, Err("not compiled in".to_owned())));
    results
}
