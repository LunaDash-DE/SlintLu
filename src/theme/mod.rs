//! Colour themes: built-in palettes, a `theme.json` in the shell's directory, pywal colours and
//! automatic light / dark switching.

mod config;
mod palette;
mod pywal;

pub use config::{ThemeConfig, resolve};
pub use palette::{Color, Palette, preset, presets};
