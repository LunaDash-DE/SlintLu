//! Vulkan rendering through wgpu, for Slint's FemtoVG wgpu renderer.

use std::cell::Cell;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::rc::Rc;

use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
};
use slint::platform::femtovg_renderer::FemtoVGWGPURenderer;
use slint::wgpu_30::wgpu;
use smithay_client_toolkit::reexports::client::Proxy;
use smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface;

use crate::core::context::Context;

/// Instance, adapter, device and queue shared by every window.
pub(crate) struct GpuContext {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

fn create_surface(
    instance: &wgpu::Instance,
    ctx: &Context,
    surface: &WlSurface,
) -> Result<wgpu::Surface<'static>, String> {
    let display =
        NonNull::new(ctx.conn.backend().display_ptr() as *mut c_void).ok_or("null wl_display")?;
    let window = NonNull::new(surface.id().as_ptr() as *mut c_void).ok_or("null wl_surface")?;
    let target = wgpu::SurfaceTargetUnsafe::RawHandle {
        raw_display_handle: Some(RawDisplayHandle::Wayland(WaylandDisplayHandle::new(display))),
        raw_window_handle: RawWindowHandle::Wayland(WaylandWindowHandle::new(window)),
    };
    // SAFETY: the connection lives as long as the platform, and the wl_surface outlives the
    // wgpu surface because `GpuWindow` is dropped before the window's role object.
    unsafe { instance.create_surface_unsafe(target) }.map_err(|e| e.to_string())
}

impl GpuContext {
    /// Returns the shared context, creating it (and the first window's surface) on first use.
    fn shared(
        ctx: &Context,
        surface: &WlSurface,
    ) -> Result<(Rc<Self>, wgpu::Surface<'static>), String> {
        if let Some(gpu) = ctx.gpu.get() {
            let gpu = gpu.clone().ok_or("Vulkan initialisation failed earlier")?;
            let wgpu_surface = create_surface(&gpu.instance, ctx, surface)?;
            return Ok((gpu, wgpu_surface));
        }
        let result = Self::new(ctx, surface);
        let _ = ctx.gpu.set(result.as_ref().ok().map(|(gpu, _)| gpu.clone()));
        result
    }

    fn new(
        ctx: &Context,
        surface: &WlSurface,
    ) -> Result<(Rc<Self>, wgpu::Surface<'static>), String> {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
        descriptor.backends = wgpu::Backends::VULKAN;
        let instance = wgpu::Instance::new(descriptor);
        let wgpu_surface = create_surface(&instance, ctx, surface)?;

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&wgpu_surface),
            ..Default::default()
        }))
        .map_err(|e| format!("no Vulkan adapter: {e}"))?;
        let info = adapter.get_info();
        log::info!(
            "Vulkan adapter: {} ({:?}, driver {})",
            info.name,
            info.device_type,
            info.driver
        );

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("slintlu"),
            ..Default::default()
        }))
        .map_err(|e| format!("cannot open the Vulkan device: {e}"))?;

        Ok((Rc::new(Self { instance, adapter, device, queue }), wgpu_surface))
    }
}

/// One window's swapchain plus its FemtoVG renderer.
pub(crate) struct GpuWindow {
    pub renderer: FemtoVGWGPURenderer,
    surface: wgpu::Surface<'static>,
    gpu: Rc<GpuContext>,
    configured_size: Cell<(u32, u32)>,
    format: wgpu::TextureFormat,
    alpha_mode: wgpu::CompositeAlphaMode,
    present_mode: wgpu::PresentMode,
}

impl GpuWindow {
    pub fn new(ctx: &Context, wl_surface: &WlSurface) -> Result<Self, String> {
        let (gpu, surface) = GpuContext::shared(ctx, wl_surface)?;
        let caps = surface.get_capabilities(&gpu.adapter);
        // FemtoVG blends in non-sRGB space; prefer a plain 8-bit format.
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| {
                matches!(f, wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm)
            })
            .or_else(|| caps.formats.first().copied())
            .ok_or("surface reports no formats")?;
        let alpha_mode =
            [wgpu::CompositeAlphaMode::PreMultiplied, wgpu::CompositeAlphaMode::Inherit]
                .into_iter()
                .find(|m| caps.alpha_modes.contains(m))
                .unwrap_or(wgpu::CompositeAlphaMode::Auto);
        // Mailbox never blocks; FIFO is always available.
        let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
            wgpu::PresentMode::Mailbox
        } else {
            wgpu::PresentMode::Fifo
        };
        let renderer =
            FemtoVGWGPURenderer::new(gpu.instance.clone(), gpu.device.clone(), gpu.queue.clone())
                .map_err(|e| e.to_string())?;
        Ok(Self {
            renderer,
            surface,
            gpu,
            configured_size: Cell::new((0, 0)),
            format,
            alpha_mode,
            present_mode,
        })
    }

    fn configure(&self, width: u32, height: u32) {
        self.surface.configure(
            &self.gpu.device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: self.format,
                width,
                height,
                present_mode: self.present_mode,
                desired_maximum_frame_latency: 2,
                alpha_mode: self.alpha_mode,
                color_space: wgpu::SurfaceColorSpace::Auto,
                view_formats: vec![],
            },
        );
        self.configured_size.set((width, height));
    }

    pub fn render(&self, width: u32, height: u32) -> Result<(), String> {
        if self.configured_size.get() != (width, height) {
            self.configure(width, height);
        }
        let texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.configure(width, height);
                return Err("swapchain was out of date, reconfigured".into());
            }
            other => return Err(format!("cannot acquire a swapchain image: {other:?}")),
        };
        self.renderer.render_to_texture(&texture.texture).map_err(|e| e.to_string())?;
        self.gpu.queue.present(texture);
        Ok(())
    }
}
