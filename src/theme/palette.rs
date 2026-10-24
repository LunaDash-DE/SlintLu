//! Palettes: the colour roles shells style themselves with.

use serde::{Deserialize, Serialize};

/// An sRGB colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Color(pub u8, pub u8, pub u8);

impl Color {
    /// `#rrggbb` or `rrggbb`.
    pub fn parse(text: &str) -> Option<Self> {
        let hex = text.trim().trim_start_matches('#');
        if hex.len() != 6 || !hex.is_ascii() {
            return None;
        }
        let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        Some(Self(channel(0)?, channel(2)?, channel(4)?))
    }

    /// Relative luminance (WCAG), 0 for black to 1 for white.
    pub fn luminance(self) -> f32 {
        let linear = |c: u8| {
            let c = c as f32 / 255.0;
            if c <= 0.039_28 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * linear(self.0) + 0.7152 * linear(self.1) + 0.0722 * linear(self.2)
    }
}

impl TryFrom<String> for Color {
    type Error = String;
    fn try_from(text: String) -> Result<Self, String> {
        Self::parse(&text).ok_or_else(|| format!("`{text}` is not a #rrggbb colour"))
    }
}

impl From<Color> for String {
    fn from(c: Color) -> String {
        format!("#{:02x}{:02x}{:02x}", c.0, c.1, c.2)
    }
}

/// Colour roles. Every theme defines all of them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Palette {
    /// Filled in from the key for palettes defined in theme.json.
    #[serde(default)]
    pub name: String,
    pub dark: bool,
    /// Window and panel background.
    pub background: Color,
    /// Cards, chips, inputs.
    pub surface: Color,
    /// Hovered or raised surfaces.
    pub surface_alt: Color,
    pub border: Color,
    pub text: Color,
    pub text_muted: Color,
    pub accent: Color,
    pub red: Color,
    pub green: Color,
    pub yellow: Color,
    pub blue: Color,
    pub magenta: Color,
    pub cyan: Color,
}

macro_rules! palette {
    ($name:literal, $dark:literal, [$($hex:literal),* $(,)?]) => {{
        let c: Vec<Color> = [$($hex),*].iter().map(|h| Color::parse(h).expect("valid preset colour")).collect();
        Palette {
            name: $name.into(), dark: $dark,
            background: c[0], surface: c[1], surface_alt: c[2], border: c[3], text: c[4], text_muted: c[5],
            accent: c[6], red: c[7], green: c[8], yellow: c[9], blue: c[10], magenta: c[11], cyan: c[12],
        }
    }};
}

/// Built-in palettes.
pub fn presets() -> Vec<Palette> {
    vec![
        palette!(
            "catppuccin-mocha",
            true,
            [
                "#1e1e2e", "#313244", "#45475a", "#585b70", "#cdd6f4", "#a6adc8", "#89b4fa",
                "#f38ba8", "#a6e3a1", "#f9e2af", "#89b4fa", "#cba6f7", "#94e2d5"
            ]
        ),
        palette!(
            "catppuccin-latte",
            false,
            [
                "#eff1f5", "#ccd0da", "#bcc0cc", "#acb0be", "#4c4f69", "#6c6f85", "#1e66f5",
                "#d20f39", "#40a02b", "#df8e1d", "#1e66f5", "#8839ef", "#179299"
            ]
        ),
        palette!(
            "nord",
            true,
            [
                "#2e3440", "#3b4252", "#434c5e", "#4c566a", "#eceff4", "#d8dee9", "#88c0d0",
                "#bf616a", "#a3be8c", "#ebcb8b", "#81a1c1", "#b48ead", "#8fbcbb"
            ]
        ),
        palette!(
            "gruvbox-dark",
            true,
            [
                "#282828", "#3c3836", "#504945", "#665c54", "#ebdbb2", "#bdae93", "#fe8019",
                "#fb4934", "#b8bb26", "#fabd2f", "#83a598", "#d3869b", "#8ec07c"
            ]
        ),
        palette!(
            "gruvbox-light",
            false,
            [
                "#fbf1c7", "#ebdbb2", "#d5c4a1", "#bdae93", "#3c3836", "#665c54", "#af3a03",
                "#9d0006", "#79740e", "#b57614", "#076678", "#8f3f71", "#427b58"
            ]
        ),
        palette!(
            "tokyo-night",
            true,
            [
                "#1a1b26", "#24283b", "#292e42", "#414868", "#c0caf5", "#a9b1d6", "#7aa2f7",
                "#f7768e", "#9ece6a", "#e0af68", "#7aa2f7", "#bb9af7", "#7dcfff"
            ]
        ),
    ]
}

/// A built-in palette by name.
pub fn preset(name: &str) -> Option<Palette> {
    presets().into_iter().find(|p| p.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_parse_and_print() {
        assert_eq!(Color::parse("#89b4fa"), Some(Color(0x89, 0xb4, 0xfa)));
        assert_eq!(Color::parse("FFFFFF"), Some(Color(255, 255, 255)));
        assert_eq!(Color::parse("#fff"), None);
        assert_eq!(String::from(Color(1, 2, 255)), "#0102ff");
    }

    #[test]
    fn presets_are_consistent() {
        for p in presets() {
            // Dark themes have dark backgrounds and light text, and the other way around.
            assert_eq!(p.background.luminance() < p.text.luminance(), p.dark, "{}", p.name);
        }
        assert!(preset("nord").is_some());
        assert!(preset("nope").is_none());
    }
}
