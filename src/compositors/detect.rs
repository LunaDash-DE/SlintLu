//! Detecting which compositor we are running under.

use std::env;

/// A compositor SlintLu knows how to talk to beyond the core Wayland protocols.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compositor {
    LunaDash,
    Niri,
    Hyprland,
    Sway,
    River,
    Kde,
    Wayfire,
    Labwc,
    Gnome,
    Unknown,
}

impl Compositor {
    /// Detects the compositor from the environment it exports to clients.
    pub fn detect() -> Self {
        Self::from_env(|name| env::var(name).ok())
    }

    fn from_env(var: impl Fn(&str) -> Option<String>) -> Self {
        // Compositor-specific IPC sockets are the most reliable signal.
        if var("LUNADASH_CONTROL").is_some() || var("LUDASH_CONTROL").is_some() {
            return Self::LunaDash;
        }
        if var("NIRI_SOCKET").is_some() {
            return Self::Niri;
        }
        if var("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
            return Self::Hyprland;
        }
        if var("SWAYSOCK").is_some() {
            return Self::Sway;
        }
        let desktop = var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_ascii_lowercase();
        desktop
            .split(':')
            .find_map(|name| match name {
                "lunadash" => Some(Self::LunaDash),
                "niri" => Some(Self::Niri),
                "hyprland" => Some(Self::Hyprland),
                "sway" => Some(Self::Sway),
                "river" => Some(Self::River),
                "kde" => Some(Self::Kde),
                "wayfire" => Some(Self::Wayfire),
                "labwc" | "wlroots" => Some(Self::Labwc),
                "gnome" => Some(Self::Gnome),
                _ => None,
            })
            .unwrap_or(Self::Unknown)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::LunaDash => "lunadash",
            Self::Niri => "niri",
            Self::Hyprland => "hyprland",
            Self::Sway => "sway",
            Self::River => "river",
            Self::Kde => "kde",
            Self::Wayfire => "wayfire",
            Self::Labwc => "labwc",
            Self::Gnome => "gnome",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for Compositor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detect(vars: &[(&str, &str)]) -> Compositor {
        Compositor::from_env(|name| {
            vars.iter().find(|(k, _)| *k == name).map(|(_, v)| v.to_string())
        })
    }

    #[test]
    fn ipc_sockets_win() {
        assert_eq!(detect(&[("NIRI_SOCKET", "/run/niri.sock")]), Compositor::Niri);
        assert_eq!(
            detect(&[("HYPRLAND_INSTANCE_SIGNATURE", "x"), ("XDG_CURRENT_DESKTOP", "sway")]),
            Compositor::Hyprland
        );
        assert_eq!(detect(&[("SWAYSOCK", "/run/sway.sock")]), Compositor::Sway);
        assert_eq!(
            detect(&[("LUNADASH_CONTROL", "/run/user/1/lunadash-0-control")]),
            Compositor::LunaDash
        );
    }

    #[test]
    fn falls_back_to_xdg_current_desktop() {
        assert_eq!(detect(&[("XDG_CURRENT_DESKTOP", "KDE")]), Compositor::Kde);
        assert_eq!(detect(&[("XDG_CURRENT_DESKTOP", "ubuntu:GNOME")]), Compositor::Gnome);
        assert_eq!(detect(&[]), Compositor::Unknown);
    }
}
