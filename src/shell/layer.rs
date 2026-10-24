//! `zwlr_layer_surface_v1` creation, configuration and events.
//!
//! The role object is SlintLu's own rather than SCTK's `LayerSurface`, which acknowledges every
//! configure as it arrives and destroys the `wl_surface` with the role. Hiding a panel destroys
//! just the role object: a configure the compositor sent before it saw the unmap then targets a
//! dead object and is dropped, instead of being acknowledged after the compositor forgot its
//! serial (a protocol error on wlroots).

use smithay_client_toolkit::compositor::Surface;
use smithay_client_toolkit::dispatch2::Dispatch2;
use smithay_client_toolkit::globals::ProvidesBoundGlobal;
use smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface;
use smithay_client_toolkit::reexports::client::{Connection, Proxy, QueueHandle};
use smithay_client_toolkit::reexports::protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{self, ZwlrLayerShellV1},
    zwlr_layer_surface_v1::{self, Anchor, KeyboardInteractivity, ZwlrLayerSurfaceV1},
};
use smithay_client_toolkit::shell::wlr_layer::{
    LayerShellHandler, LayerSurface, LayerSurfaceConfigure,
};

use super::config::{KeyboardMode, Layer, LayerConfig};
use crate::core::context::Context;
use crate::core::state::State;

fn wlr_layer(layer: Layer) -> zwlr_layer_shell_v1::Layer {
    use zwlr_layer_shell_v1::Layer as L;
    match layer {
        Layer::Background => L::Background,
        Layer::Bottom => L::Bottom,
        Layer::Top => L::Top,
        Layer::Overlay => L::Overlay,
    }
}

fn wlr_anchor(config: &LayerConfig) -> Anchor {
    let a = config.anchors;
    let mut anchor = Anchor::empty();
    anchor.set(Anchor::Top, a.top);
    anchor.set(Anchor::Bottom, a.bottom);
    anchor.set(Anchor::Left, a.left);
    anchor.set(Anchor::Right, a.right);
    anchor
}

fn wlr_keyboard(mode: KeyboardMode) -> KeyboardInteractivity {
    match mode {
        KeyboardMode::None => KeyboardInteractivity::None,
        KeyboardMode::Exclusive => KeyboardInteractivity::Exclusive,
        KeyboardMode::OnDemand => KeyboardInteractivity::OnDemand,
    }
}

/// The size to request from the compositor: zero along axes the compositor stretches.
pub(crate) fn requested_size(config: &LayerConfig, preferred: (u32, u32)) -> (u32, u32) {
    let width = if config.anchors.stretches_horizontally() { 0 } else { preferred.0.max(1) };
    let height = if config.anchors.stretches_vertically() { 0 } else { preferred.1.max(1) };
    (width, height)
}

/// A `zwlr_layer_surface_v1` role object; dropping it destroys the role but not the surface.
pub(crate) struct LayerRole {
    proxy: ZwlrLayerSurfaceV1,
}

impl Drop for LayerRole {
    fn drop(&mut self) {
        self.proxy.destroy();
    }
}

/// Finds the window of a layer surface's events.
pub(crate) struct LayerData {
    wl_surface: WlSurface,
}

/// Creates the layer surface. Returns `None` if layer-shell is unavailable.
pub(crate) fn create(ctx: &Context, surface: &Surface, config: &LayerConfig) -> Option<LayerRole> {
    let layer_shell = ctx.layer_shell.as_ref()?;
    let shell: ZwlrLayerShellV1 =
        ProvidesBoundGlobal::<ZwlrLayerShellV1, 4>::bound_global(layer_shell).ok()?;
    let output = config.output.and_then(|id| crate::output::wl_output_by_id(ctx, id));
    if config.output.is_some() && output.is_none() {
        log::warn!("output {:?} is gone, letting the compositor choose", config.output);
    }
    let wl_surface = surface.wl_surface().clone();
    let proxy = shell.get_layer_surface(
        &wl_surface,
        output.as_ref(),
        wlr_layer(config.layer),
        config.namespace.clone(),
        &ctx.qh,
        LayerData { wl_surface: wl_surface.clone() },
    );
    Some(LayerRole { proxy })
}

/// Sends the (double-buffered) configuration; the caller commits.
pub(crate) fn apply(
    layer: &LayerRole,
    config: &LayerConfig,
    preferred: (u32, u32),
    current: (u32, u32),
) {
    let layer = &layer.proxy;
    let (width, height) = requested_size(config, preferred);
    if layer.version() >= 2 {
        layer.set_layer(wlr_layer(config.layer));
    }
    layer.set_anchor(wlr_anchor(config));
    layer.set_size(width, height);
    let (zone_w, zone_h) = if current.0 > 0 && current.1 > 0 { current } else { preferred };
    layer.set_exclusive_zone(config.exclusive_zone.resolve(config.anchors, zone_w, zone_h));
    let m = config.margins;
    layer.set_margin(m.top, m.right, m.bottom, m.left);
    let keyboard = match config.keyboard {
        // on-demand needs version 4; fall back to exclusive focus when clicked is impossible.
        KeyboardMode::OnDemand if layer.version() < 4 => KeyboardInteractivity::Exclusive,
        mode => wlr_keyboard(mode),
    };
    layer.set_keyboard_interactivity(keyboard);
}

/// Sets the exclusive zone, e.g. to follow the configured size.
pub(crate) fn set_exclusive_zone(layer: &LayerRole, zone: i32) {
    layer.proxy.set_exclusive_zone(zone);
}

// SCTK binds the layer-shell global; its own layer surfaces are never created.
impl LayerShellHandler for State {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {}

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &LayerSurface,
        _: LayerSurfaceConfigure,
        _: u32,
    ) {
    }
}

impl Dispatch2<ZwlrLayerSurfaceV1, State> for LayerData {
    fn event(
        &self,
        state: &mut State,
        proxy: &ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
        match event {
            zwlr_layer_surface_v1::Event::Configure { serial, width, height } => {
                proxy.ack_configure(serial);
                if let Some(window) = state.ctx.find_window(&self.wl_surface) {
                    window.configure(width, height);
                }
            }
            zwlr_layer_surface_v1::Event::Closed => {
                if let Some(window) = state.ctx.find_window(&self.wl_surface) {
                    window.closed_by_compositor();
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::config::Anchors;

    #[test]
    fn stretched_axes_request_zero() {
        let bar = LayerConfig { anchors: Anchors::TOP_EDGE, ..LayerConfig::default() };
        assert_eq!(requested_size(&bar, (800, 32)), (0, 32));
        let panel = LayerConfig { anchors: Anchors::LEFT_EDGE, ..LayerConfig::default() };
        assert_eq!(requested_size(&panel, (48, 600)), (48, 0));
        let fullscreen = LayerConfig { anchors: Anchors::ALL, ..LayerConfig::default() };
        assert_eq!(requested_size(&fullscreen, (1, 1)), (0, 0));
    }

    #[test]
    fn floating_surfaces_never_request_zero() {
        let popup = LayerConfig::default();
        assert_eq!(requested_size(&popup, (0, 0)), (1, 1));
        assert_eq!(requested_size(&popup, (300, 200)), (300, 200));
    }
}
