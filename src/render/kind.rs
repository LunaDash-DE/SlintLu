//! Selection of the Slint renderer / graphics API.

use std::str::FromStr;

/// Which Slint renderer (and therefore which graphics API) windows use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RendererKind {
    /// Try Vulkan, then OpenGL, then fall back to the software renderer.
    #[default]
    Auto,
    /// CPU rendering into `wl_shm` buffers. Always available.
    Software,
    /// FemtoVG on OpenGL ES through EGL.
    OpenGL,
    /// FemtoVG on wgpu with the Vulkan backend.
    Vulkan,
}

impl RendererKind {
    /// The renderers to try, in order, when this kind is requested.
    pub(crate) fn candidates(self) -> &'static [RendererKind] {
        match self {
            RendererKind::Auto => {
                &[RendererKind::Vulkan, RendererKind::OpenGL, RendererKind::Software]
            }
            RendererKind::Software => &[RendererKind::Software],
            RendererKind::OpenGL => &[RendererKind::OpenGL, RendererKind::Software],
            RendererKind::Vulkan => &[RendererKind::Vulkan, RendererKind::Software],
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            RendererKind::Auto => "auto",
            RendererKind::Software => "software",
            RendererKind::OpenGL => "opengl",
            RendererKind::Vulkan => "vulkan",
        }
    }
}

impl FromStr for RendererKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "auto" | "" => Ok(Self::Auto),
            "software" | "sw" | "cpu" => Ok(Self::Software),
            "opengl" | "gl" | "gles" | "egl" | "femtovg" => Ok(Self::OpenGL),
            "vulkan" | "vk" | "wgpu" => Ok(Self::Vulkan),
            other => Err(format!(
                "unknown renderer `{other}` (expected auto, software, opengl or vulkan)"
            )),
        }
    }
}

impl std::fmt::Display for RendererKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_kind_parses_aliases() {
        assert_eq!("VK".parse::<RendererKind>().unwrap(), RendererKind::Vulkan);
        assert_eq!("gl".parse::<RendererKind>().unwrap(), RendererKind::OpenGL);
        assert_eq!("software".parse::<RendererKind>().unwrap(), RendererKind::Software);
        assert_eq!("".parse::<RendererKind>().unwrap(), RendererKind::Auto);
        assert!("metal".parse::<RendererKind>().is_err());
    }

    #[test]
    fn renderers_always_fall_back_to_software() {
        for kind in
            [RendererKind::Auto, RendererKind::Software, RendererKind::OpenGL, RendererKind::Vulkan]
        {
            assert_eq!(kind.candidates().last(), Some(&RendererKind::Software));
        }
    }
}
