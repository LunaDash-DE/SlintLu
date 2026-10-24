//! CPU rendering into shared-memory buffers. Works on every compositor and in CI.

use std::cell::RefCell;

use slint::platform::software_renderer::{RepaintBufferType, SoftwareRenderer};
use smithay_client_toolkit::reexports::client::protocol::wl_shm;
use smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface;
use smithay_client_toolkit::shm::Shm;
use smithay_client_toolkit::shm::slot::SlotPool;

use super::pixel::Argb8888;

pub(crate) struct SoftwareBackend {
    pub renderer: SoftwareRenderer,
    pool: RefCell<Option<SlotPool>>,
}

impl SoftwareBackend {
    pub fn new() -> Self {
        Self {
            // Buffers come from a pool and may hold any older frame, so repaint everything.
            renderer: SoftwareRenderer::new_with_repaint_buffer_type(RepaintBufferType::NewBuffer),
            pool: RefCell::new(None),
        }
    }

    pub fn render(
        &self,
        shm: &Shm,
        surface: &WlSurface,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        let stride = width as usize * 4;
        let len = stride * height as usize;

        let mut pool = self.pool.borrow_mut();
        let pool = match &mut *pool {
            Some(pool) => pool,
            None => pool.insert(SlotPool::new(len, shm).map_err(|e| e.to_string())?),
        };
        let (buffer, canvas) = pool
            .create_buffer(width as i32, height as i32, stride as i32, wl_shm::Format::Argb8888)
            .map_err(|e| e.to_string())?;

        let canvas = &mut canvas[..len];
        canvas.fill(0);
        let pixels: &mut [Argb8888] = bytemuck::cast_slice_mut(canvas);
        self.renderer.render(pixels, width as usize);

        surface.damage_buffer(0, 0, width as i32, height as i32);
        buffer.attach_to(surface).map_err(|e| e.to_string())?;
        surface.commit();
        Ok(())
    }
}
