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
        let scale = frame.scale.get().max(1);
        let (pw, ph) = (w * scale as u32, h * scale as u32);
        if pw == 0 || ph == 0 {
            return;
        }

        // Both are double-buffered and take effect with the commit done by the renderer.
        self.wl_surface.set_buffer_scale(scale);
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
