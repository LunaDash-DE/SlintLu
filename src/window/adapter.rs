//! The [`WindowAdapter`] trait implementation.

use slint::PhysicalSize;
use slint::platform::{PlatformError, Renderer, WindowAdapter, WindowProperties};

use super::WaylandWindow;

impl WindowAdapter for WaylandWindow {
    fn window(&self) -> &slint::Window {
        &self.window
    }

    fn set_visible(&self, visible: bool) -> Result<(), PlatformError> {
        if visible {
            self.map()
        } else {
            self.unmap()
        }
        Ok(())
    }

    fn size(&self) -> PhysicalSize {
        let (w, h) = self.frame.logical_size.get();
        let scale = self.frame.scale.get();
        PhysicalSize::new(super::scale::physical(w, scale), super::scale::physical(h, scale))
    }

    fn renderer(&self) -> &dyn Renderer {
        self.renderer.as_slint()
    }

    fn request_redraw(&self) {
        self.frame.dirty.set(true);
    }

    fn update_window_properties(&self, properties: WindowProperties<'_>) {
        let title = properties.title().to_string();
        let constraints = properties.layout_constraints();
        let to_px = |s: slint::LogicalSize| (s.width.ceil() as u32, s.height.ceil() as u32);
        let preferred = to_px(constraints.preferred);
        let min = constraints.min.map(to_px);
        self.properties_changed(title, preferred, min);
    }
}
