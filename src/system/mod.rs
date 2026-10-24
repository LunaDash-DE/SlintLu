//! Machine statistics read from `/proc` and `/sys`: CPU, memory, disks, temperatures,
//! batteries, network traffic and host information.

mod backlight;
mod battery;
mod cpu;
mod disk;
mod host;
mod memory;
mod network;
mod thermal;

pub use backlight::{Backlight, backlight};
pub use battery::{Battery, BatteryState, batteries};
pub use cpu::CpuSampler;
pub use disk::{DiskUsage, disk_usage};
pub use host::{HostInfo, host_info, uptime};
pub use memory::{MemoryInfo, memory};
pub use network::{NetworkSampler, NetworkTraffic};
pub use thermal::{Temperature, temperatures};
