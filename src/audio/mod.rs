//! Audio devices and volume through `pactl`, which talks to both PipeWire (pipewire-pulse) and
//! PulseAudio.

mod model;
mod pactl;

pub use model::{AudioState, Device, DeviceKind};
pub use pactl::Pactl;
