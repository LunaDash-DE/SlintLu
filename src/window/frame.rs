//! Frame pacing: draw only when something changed and the compositor is ready for a frame.

use smithay_client_toolkit::compositor::FrameCallbackData;

use super::WaylandWindow;

impl WaylandWindow {
    pub(crate) fn render_if_needed(&self) {
        let frame = &self.frame;
        if !frame.shown.get() || !frame.configured.get() || frame.frame_pending.get() {
            return;
        }
        if !frame.dirty.replace(false) {
            return;
        }
        let (w, h) = frame.logical_size.get();
        let scale = frame.scale.get();
        let (pw, ph) = (super::scale::physical(w, scale), super::scale::physical(h, scale));
        if pw == 0 || ph == 0 {
            return;
        }

        // Double-buffered state, applied by the renderer's commit: with a viewport the buffer
        // is mapped onto the logical size; otherwise integer buffer scaling.
        match &self.scaling {
            Some(scaling) => {
                self.wl_surface.set_buffer_scale(1);
                scaling.viewport.set_destination(w as i32, h as i32);
            }
            None => self.wl_surface.set_buffer_scale(scale.round().max(1.0) as i32),
        }
        self.wl_surface.frame(&self.ctx.qh, FrameCallbackData(self.wl_surface.clone()));
        frame.frame_pending.set(true);

        if let Err(e) = self.renderer.render(&self.ctx, &self.wl_surface, pw, ph) {
            log::error!("rendering failed: {e}");
            frame.frame_pending.set(false);
        }
    }

    pub(crate) fn frame_done(&self) {
        self.frame.frame_pending.set(false);
        if self.window.has_active_animations() {
            self.frame.dirty.set(true);
        }
    }
}
