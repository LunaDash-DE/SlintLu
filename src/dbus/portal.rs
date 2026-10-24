//! Appearance settings (dark mode, accent colour) from the XDG desktop portal.

use futures_util::StreamExt;
use zbus::{Connection, OwnedVariant, Variant, proxy};

const APPEARANCE: &str = "org.freedesktop.appearance";

#[proxy(
    interface = "org.freedesktop.portal.Settings",
    default_service = "org.freedesktop.portal.Desktop",
    default_path = "/org/freedesktop/portal/desktop"
)]
trait Settings {
    fn read_one(&self, namespace: &str, key: &str) -> zbus::Result<OwnedVariant>;
    #[zbus(signal)]
    fn setting_changed(&self, namespace: &str, key: &str, value: Variant<'_>) -> zbus::Result<()>;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorScheme {
    #[default]
    NoPreference,
    Dark,
    Light,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Appearance {
    pub color_scheme: ColorScheme,
    /// sRGB in 0..1, if the desktop provides one.
    pub accent: Option<(f32, f32, f32)>,
    pub high_contrast: bool,
}

fn color_scheme(value: &Variant<'_>) -> ColorScheme {
    match u32::try_from(value) {
        Ok(1) => ColorScheme::Dark,
        Ok(2) => ColorScheme::Light,
        _ => ColorScheme::NoPreference,
    }
}

fn accent(value: &Variant<'_>) -> Option<(f32, f32, f32)> {
    let (r, g, b) = <(f64, f64, f64)>::try_from(value).ok()?;
    let valid = |c: f64| (0.0..=1.0).contains(&c);
    (valid(r) && valid(g) && valid(b)).then_some((r as f32, g as f32, b as f32))
}

fn apply(appearance: &mut Appearance, key: &str, value: &Variant<'_>) -> bool {
    match key {
        "color-scheme" => appearance.color_scheme = color_scheme(value),
        "accent-color" => appearance.accent = accent(value),
        "contrast" => appearance.high_contrast = u32::try_from(value) == Ok(1),
        _ => return false,
    }
    true
}

#[derive(Clone)]
pub struct Portal {
    connection: Connection,
}

impl Portal {
    pub fn connect() -> zbus::Result<Self> {
        Ok(Self { connection: zbus::block_on(Connection::session())? })
    }

    async fn read(&self) -> zbus::Result<Appearance> {
        let settings = SettingsProxy::new(&self.connection).await?;
        let mut appearance = Appearance::default();
        for key in ["color-scheme", "accent-color", "contrast"] {
            if let Ok(value) = settings.read_one(APPEARANCE, key).await {
                apply(&mut appearance, key, &value);
            }
        }
        Ok(appearance)
    }

    pub fn appearance(&self) -> zbus::Result<Appearance> {
        zbus::block_on(self.read())
    }

    /// Blocks, calling `on_change` initially and whenever an appearance setting changes.
    pub fn watch(&self, mut on_change: impl FnMut(&Appearance)) -> zbus::Result<()> {
        zbus::block_on(async {
            let settings = SettingsProxy::new(&self.connection).await?;
            let mut changes = settings.receive_setting_changed().await?;
            let mut appearance = self.read().await?;
            on_change(&appearance);
            while let Some(signal) = changes.next().await {
                let Ok(args) = signal.args() else { continue };
                if args.namespace == APPEARANCE && apply(&mut appearance, args.key, &args.value) {
                    on_change(&appearance);
                }
            }
            Err(zbus::Error::Failure("the session bus closed".into()))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_settings() {
        let mut appearance = Appearance::default();
        assert!(apply(&mut appearance, "color-scheme", &Variant::from(1u32)));
        assert_eq!(appearance.color_scheme, ColorScheme::Dark);
        assert!(apply(&mut appearance, "accent-color", &Variant::from((0.2f64, 0.4f64, 1.0f64))));
        assert_eq!(appearance.accent, Some((0.2, 0.4, 1.0)));
        assert!(apply(&mut appearance, "accent-color", &Variant::from((-1.0f64, 0.0f64, 0.0f64))));
        assert_eq!(appearance.accent, None);
        assert!(!apply(&mut appearance, "reduced-motion", &Variant::from(1u32)));
    }
}
