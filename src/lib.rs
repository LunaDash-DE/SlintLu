//! A [Slint](https://slint.dev) platform backend for building Wayland desktop shells.
//!
//! `slintlu-wayland` replaces Slint's default (winit) backend with one that talks to the
//! compositor directly through [smithay-client-toolkit], which gives Slint windows access to
//! shell-only protocols such as `wlr-layer-shell` (bars, docks, wallpapers, overlays,
//! notifications, launchers …) next to ordinary `xdg-shell` toplevel windows.
//!
//! Rendering is done by one of Slint's renderers:
//!
//! | [`RendererKind`] | Renderer | Graphics API |
//! |---|---|---|
//! | `Software` | Slint software renderer | `wl_shm` shared memory, no GPU needed |
//! | `OpenGL` | FemtoVG | OpenGL ES 2/3 through EGL (`libEGL` loaded at runtime) |
//! | `Vulkan` | FemtoVG on wgpu | Vulkan |
//! | `Auto` | first one that works | Vulkan → OpenGL → Software |
//!
//! # Using it from Rust
//!
//! ```no_run
//! use slintlu::{Anchors, LayerConfig, PlatformConfig, SurfaceRole};
//!
//! slint::slint! {
//!     export component Bar inherits Window {
//!         preferred-height: 32px;
//!         Text { text: "hello from a layer surface"; }
//!     }
//! }
//!
//! slintlu::init(PlatformConfig::default()).unwrap();
//! let bar = Bar::new().unwrap();
//! slintlu::window_for(slint::ComponentHandle::window(&bar))
//!     .unwrap()
//!     .set_role(SurfaceRole::Layer(LayerConfig {
//!         anchors: Anchors::TOP_EDGE,
//!         ..LayerConfig::default()
//!     }));
//! slint::ComponentHandle::run(&bar).unwrap();
//! ```
//!
//! [smithay-client-toolkit]: https://docs.rs/smithay-client-toolkit

pub mod audio;
pub mod compositors;
mod core;
mod input;
mod output;
mod render;
mod shell;
pub mod system;
pub mod time;
pub mod util;
mod window;

pub use core::{PlatformConfig, WaylandPlatform, init, window_for};
pub use output::{OutputInfo, on_outputs_changed, outputs};
pub use render::RendererKind;
pub use shell::config::{
    Anchors, ExclusiveZone, KeyboardMode, Layer, LayerConfig, Margins, SurfaceRole,
};
pub use window::WaylandWindow;
