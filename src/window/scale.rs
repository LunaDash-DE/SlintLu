//! Fractional scaling (`wp_fractional_scale_v1`) with `wp_viewporter`: windows render at the
//! exact scale of their output (e.g. 1.25) instead of rounding up and being downscaled.

use smithay_client_toolkit::dispatch2::Dispatch2;
use smithay_client_toolkit::reexports::client::globals::GlobalList;
use smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface;
use smithay_client_toolkit::reexports::client::{Connection, QueueHandle};
use smithay_client_toolkit::reexports::protocols::wp::fractional_scale::v1::client::{
    wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
    wp_fractional_scale_v1::{self, WpFractionalScaleV1},
};
use smithay_client_toolkit::reexports::protocols::wp::viewporter::client::{
    wp_viewport::{self, WpViewport},
    wp_viewporter::{self, WpViewporter},
};

use crate::core::state::State;

/// The globals needed for fractional scaling; both or nothing.
pub(crate) struct FractionalScaling {
    manager: WpFractionalScaleManagerV1,
    viewporter: WpViewporter,
}

/// Per-window objects, destroyed with the window.
pub(crate) struct SurfaceScaling {
    fractional: WpFractionalScaleV1,
    pub viewport: WpViewport,
}

impl Drop for SurfaceScaling {
    fn drop(&mut self) {
        self.fractional.destroy();
        self.viewport.destroy();
    }
}

/// User data for objects that never send events.
pub(crate) struct NoEvents;

/// User data of a surface's fractional scale object.
pub(crate) struct FractionalScaleData {
    surface: WlSurface,
}

impl FractionalScaling {
    pub fn bind(globals: &GlobalList, qh: &QueueHandle<State>) -> Option<Self> {
        let manager =
            globals.bind::<WpFractionalScaleManagerV1, State, NoEvents>(qh, 1..=1, NoEvents).ok();
        let viewporter = globals.bind::<WpViewporter, State, NoEvents>(qh, 1..=1, NoEvents).ok();
        match (manager, viewporter) {
            (Some(manager), Some(viewporter)) => Some(Self { manager, viewporter }),
            _ => {
                log::info!("fractional-scale-v1 or viewporter unavailable: using integer scaling");
                None
            }
        }
    }

    pub fn for_surface(&self, surface: &WlSurface, qh: &QueueHandle<State>) -> SurfaceScaling {
        SurfaceScaling {
            fractional: self.manager.get_fractional_scale(
                surface,
                qh,
                FractionalScaleData { surface: surface.clone() },
            ),
            viewport: self.viewporter.get_viewport(surface, qh, NoEvents),
        }
    }
}

impl Dispatch2<WpFractionalScaleV1, State> for FractionalScaleData {
    fn event(
        &self,
        state: &mut State,
        _: &WpFractionalScaleV1,
        event: wp_fractional_scale_v1::Event,
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = event
            && let Some(window) = state.ctx.find_window(&self.surface)
        {
            // The scale is sent in 120ths.
            window.set_fractional_scale(scale as f32 / 120.0);
        }
    }
}

macro_rules! no_events {
    ($($interface:ty => $module:ident),* $(,)?) => {$(
        impl Dispatch2<$interface, State> for NoEvents {
            fn event(&self, _: &mut State, _: &$interface, _: $module::Event, _: &Connection, _: &QueueHandle<State>) {}
        }
    )*};
}

use smithay_client_toolkit::reexports::protocols::wp::fractional_scale::v1::client::wp_fractional_scale_manager_v1;
no_events!(
    WpFractionalScaleManagerV1 => wp_fractional_scale_manager_v1,
    WpViewporter => wp_viewporter,
    WpViewport => wp_viewport,
);

/// Physical pixels for a logical length at a scale, as the compositor computes them.
pub(crate) fn physical(logical: u32, scale: f32) -> u32 {
    (logical as f32 * scale).round() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physical_sizes_round() {
        assert_eq!(physical(1920, 1.0), 1920);
        assert_eq!(physical(1536, 1.25), 1920);
        assert_eq!(physical(33, 1.5), 50);
        assert_eq!(physical(34, 1.75), 60);
    }
}
