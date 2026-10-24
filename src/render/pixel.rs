//! The pixel format of `wl_shm` `ARGB8888` buffers, for the software renderer.

use slint::platform::software_renderer::{PremultipliedRgbaColor, TargetPixel};

/// One premultiplied `ARGB8888` pixel, which is stored as B, G, R, A bytes in memory on
/// little-endian machines (Wayland formats are little-endian by definition).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Argb8888 {
    pub b: u8,
    pub g: u8,
    pub r: u8,
    pub a: u8,
}

fn scale(channel: u8, factor: u16) -> u8 {
    ((channel as u16 * factor + 127) / 255) as u8
}

impl TargetPixel for Argb8888 {
    fn blend(&mut self, color: PremultipliedRgbaColor) {
        let inverse = (u8::MAX - color.alpha) as u16;
        self.r = scale(self.r, inverse).saturating_add(color.red);
        self.g = scale(self.g, inverse).saturating_add(color.green);
        self.b = scale(self.b, inverse).saturating_add(color.blue);
        self.a = scale(self.a, inverse).saturating_add(color.alpha);
    }

    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self { b, g, r, a: u8::MAX }
    }

    /// Fully transparent, so that shells can have see-through areas.
    fn background() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn color(red: u8, green: u8, blue: u8, alpha: u8) -> PremultipliedRgbaColor {
        PremultipliedRgbaColor { red, green, blue, alpha }
    }

    #[test]
    fn memory_layout_is_bgra() {
        let px = Argb8888::from_rgb(1, 2, 3);
        assert_eq!(bytemuck::bytes_of(&px), &[3, 2, 1, 255]);
    }

    #[test]
    fn opaque_blend_replaces() {
        let mut px = Argb8888::from_rgb(10, 20, 30);
        px.blend(color(200, 100, 50, 255));
        assert_eq!(px, Argb8888 { r: 200, g: 100, b: 50, a: 255 });
    }

    #[test]
    fn half_transparent_over_transparent_keeps_premultiplied_values() {
        let mut px = Argb8888::background();
        px.blend(color(64, 0, 0, 128));
        assert_eq!(px, Argb8888 { r: 64, g: 0, b: 0, a: 128 });
    }

    #[test]
    fn blending_never_overflows() {
        let mut px = Argb8888::from_rgb(255, 255, 255);
        px.blend(color(255, 255, 255, 1));
        assert_eq!(px.a, 255);
    }
}
