//! `zwlr_layer_surface_v1` creation, configuration and events.

use smithay_client_toolkit::compositor::Surface;
use smithay_client_toolkit::reexports::client::{Connection, QueueHandle};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shell::wlr_layer::{
    Anchor, KeyboardInteractivity, Layer as WlrLayer, LayerShellHandler, LayerSurface,
    LayerSurfaceConfigure,
};

use super::config::{KeyboardMode, Layer, LayerConfig};
use crate::core::context::Context;
use crate::core::state::State;

fn wlr_layer(layer: Layer) -> WlrLayer {
    match layer {
        Layer::Background => WlrLayer::Background,
        Layer::Bottom => WlrLayer::Bottom,
        Layer::Top => WlrLayer::Top,
        Layer::Overlay => WlrLayer::Overlay,
    }
}

fn wlr_anchor(config: &LayerConfig) -> Anchor {
    let a = config.anchors;
    let mut anchor = Anchor::empty();
    anchor.set(Anchor::TOP, a.top);
    anchor.set(Anchor::BOTTOM, a.bottom);
    anchor.set(Anchor::LEFT, a.left);
    anchor.set(Anchor::RIGHT, a.right);
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

/// Creates the layer surface. Returns the surface back if layer-shell is unavailable.
pub(crate) fn create(
    ctx: &Context,
    surface: Surface,
    config: &LayerConfig,
) -> Result<LayerSurface, Surface> {
    let Some(layer_shell) = &ctx.layer_shell else { return Err(surface) };
    let output = config.output.and_then(|id| crate::output::wl_output_by_id(ctx, id));
    if config.output.is_some() && output.is_none() {
        log::warn!("output {:?} is gone, letting the compositor choose", config.output);
    }
    Ok(layer_shell.create_layer_surface(
        &ctx.qh,
        surface,
        wlr_layer(config.layer),
        Some(config.namespace.clone()),
        output.as_ref(),
    ))
}

/// Sends the (double-buffered) configuration; the caller commits.
pub(crate) fn apply(
    layer: &LayerSurface,
    config: &LayerConfig,
    preferred: (u32, u32),
    current: (u32, u32),
) {
    let (width, height) = requested_size(config, preferred);
    layer.set_layer(wlr_layer(config.layer));
    layer.set_anchor(wlr_anchor(config));
    layer.set_size(width, height);
    let (zone_w, zone_h) = if current.0 > 0 && current.1 > 0 { current } else { preferred };
    layer.set_exclusive_zone(config.exclusive_zone.resolve(config.anchors, zone_w, zone_h));
    let m = config.margins;
    layer.set_margin(m.top, m.right, m.bottom, m.left);
    layer.set_keyboard_interactivity(wlr_keyboard(config.keyboard));
}

impl LayerShellHandler for State {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface) {
        if let Some(window) = self.ctx.find_window(layer.wl_surface()) {
            window.closed_by_compositor();
        }
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _: u32,
    ) {
        if let Some(window) = self.ctx.find_window(layer.wl_surface()) {
            window.configure(configure.new_size.0, configure.new_size.1);
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
