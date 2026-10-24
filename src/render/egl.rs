//! OpenGL ES through EGL on Wayland, for Slint's FemtoVG renderer.
//!
//! `libEGL.so.1` is loaded at runtime, so the binary still starts (with the software renderer)
//! on machines without any GL driver.

use std::ffi::{CStr, c_void};
use std::num::NonZeroU32;
use std::rc::Rc;

use khronos_egl as egl;
use slint::platform::femtovg_renderer::OpenGLInterface;
use smithay_client_toolkit::reexports::client::Proxy;
use smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface;
use wayland_egl::WlEglSurface;

use crate::core::context::Context;

type Error = Box<dyn std::error::Error + Send + Sync>;
type Egl = egl::DynamicInstance<egl::EGL1_4>;

/// `EGL_PLATFORM_WAYLAND_KHR`
const PLATFORM_WAYLAND: egl::Enum = 0x31D8;

/// The process-wide EGL display, shared by all windows.
pub(crate) struct EglDisplay {
    egl: Egl,
    display: egl::Display,
    config: egl::Config,
}

impl EglDisplay {
    /// Initialises EGL once per connection; later calls return the cached result.
    pub fn shared(ctx: &Context) -> Option<Rc<Self>> {
        ctx.egl
            .get_or_init(|| {
                Self::new(ctx)
                    .inspect_err(|e| log::warn!("cannot initialise EGL: {e}"))
                    .ok()
                    .map(Rc::new)
            })
            .clone()
    }

    fn new(ctx: &Context) -> Result<Self, String> {
        // SAFETY: loading the system EGL library; its initialisers have no preconditions.
        let egl =
            unsafe { Egl::load_required() }.map_err(|e| format!("cannot load libEGL: {e}"))?;
        let native = ctx.conn.backend().display_ptr() as *mut c_void;

        // Prefer the EGL 1.5 platform API; fall back to the legacy entry point which Mesa and
        // NVIDIA also accept for a `wl_display*`.
        let display = match egl.upcast::<egl::EGL1_5>() {
            Some(egl15) => {
                // SAFETY: `native` is the live `wl_display*` of our connection.
                unsafe { egl15.get_platform_display(PLATFORM_WAYLAND, native, &[egl::ATTRIB_NONE]) }
                    .map_err(|e| e.to_string())?
            }
            // SAFETY: as above.
            None => unsafe { egl.get_display(native) }.ok_or("no EGL display for wayland")?,
        };
        egl.initialize(display).map_err(|e| format!("eglInitialize: {e}"))?;
        egl.bind_api(egl::OPENGL_ES_API).map_err(|e| format!("eglBindAPI: {e}"))?;

        let attributes = [
            egl::SURFACE_TYPE,
            egl::WINDOW_BIT,
            egl::RENDERABLE_TYPE,
            egl::OPENGL_ES2_BIT,
            egl::RED_SIZE,
            8,
            egl::GREEN_SIZE,
            8,
            egl::BLUE_SIZE,
            8,
            egl::ALPHA_SIZE,
            8,
            // FemtoVG fills paths through the stencil buffer.
            egl::STENCIL_SIZE,
            8,
            egl::NONE,
        ];
        let config = egl
            .choose_first_config(display, &attributes)
            .map_err(|e| format!("eglChooseConfig: {e}"))?
            .ok_or("no RGBA8 + stencil EGL config")?;
        Ok(Self { egl, display, config })
    }

    fn create_context(&self) -> Result<egl::Context, String> {
        [3, 2]
            .into_iter()
            .find_map(|version| {
                self.egl
                    .create_context(
                        self.display,
                        self.config,
                        None,
                        &[egl::CONTEXT_CLIENT_VERSION, version, egl::NONE],
                    )
                    .ok()
            })
            .ok_or_else(|| "cannot create an OpenGL ES 2 or 3 context".into())
    }
}

/// A GL context bound to one window's surface.
pub(crate) struct EglWindow {
    display: Rc<EglDisplay>,
    context: egl::Context,
    surface: egl::Surface,
    // Dropped after the EGL surface that renders into it.
    native: WlEglSurface,
}

impl EglWindow {
    pub fn new(display: Rc<EglDisplay>, wl_surface: &WlSurface) -> Result<Self, String> {
        let native = WlEglSurface::new(wl_surface.id(), 1, 1).map_err(|e| e.to_string())?;
        let context = display.create_context()?;
        // SAFETY: `native` is a valid wl_egl_window that outlives the EGL surface (field order).
        let surface = unsafe {
            display.egl.create_window_surface(
                display.display,
                display.config,
                native.ptr() as egl::NativeWindowType,
                None,
            )
        }
        .map_err(|e| format!("eglCreateWindowSurface: {e}"))?;

        let this = Self { display, context, surface, native };
        this.ensure_current().map_err(|e| e.to_string())?;
        // Frame pacing is done with wl_surface frame callbacks; never block in eglSwapBuffers.
        let _ = this.display.egl.swap_interval(this.display.display, 0);
        Ok(this)
    }
}

// SAFETY: every function forwards to EGL, and `get_proc_address` returns EGL's own pointers.
unsafe impl OpenGLInterface for EglWindow {
    fn ensure_current(&self) -> Result<(), Error> {
        let d = &self.display;
        d.egl.make_current(
            d.display,
            Some(self.surface),
            Some(self.surface),
            Some(self.context),
        )?;
        Ok(())
    }

    fn swap_buffers(&self) -> Result<(), Error> {
        self.display.egl.swap_buffers(self.display.display, self.surface)?;
        Ok(())
    }

    fn resize(&self, width: NonZeroU32, height: NonZeroU32) -> Result<(), Error> {
        self.native.resize(width.get() as i32, height.get() as i32, 0, 0);
        Ok(())
    }

    fn get_proc_address(&self, name: &CStr) -> *const c_void {
        name.to_str()
            .ok()
            .and_then(|name| self.display.egl.get_proc_address(name))
            .map_or(std::ptr::null(), |f| f as *const c_void)
    }
}

impl Drop for EglWindow {
    fn drop(&mut self) {
        let d = &self.display;
        let _ = d.egl.make_current(d.display, None, None, None);
        let _ = d.egl.destroy_surface(d.display, self.surface);
        let _ = d.egl.destroy_context(d.display, self.context);
    }
}
