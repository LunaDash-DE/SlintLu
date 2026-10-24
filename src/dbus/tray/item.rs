//! Reading `org.kde.StatusNotifierItem` objects.

use zbus::{Connection, OwnedObjectPath, proxy};

#[proxy(interface = "org.kde.StatusNotifierItem")]
pub(super) trait StatusNotifierItem {
    fn activate(&self, x: i32, y: i32) -> zbus::Result<()>;
    fn secondary_activate(&self, x: i32, y: i32) -> zbus::Result<()>;
    fn context_menu(&self, x: i32, y: i32) -> zbus::Result<()>;
    fn scroll(&self, delta: i32, orientation: &str) -> zbus::Result<()>;
    #[zbus(property)]
    fn id(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn title(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn status(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn icon_name(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn icon_theme_path(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn icon_pixmap(&self) -> zbus::Result<Vec<(i32, i32, Vec<u8>)>>;
    #[zbus(property)]
    fn attention_icon_name(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn tool_tip(&self) -> zbus::Result<(String, Vec<(i32, i32, Vec<u8>)>, String, String)>;
    #[zbus(property)]
    fn menu(&self) -> zbus::Result<OwnedObjectPath>;
    #[zbus(property)]
    fn item_is_menu(&self) -> zbus::Result<bool>;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TrayStatus {
    Passive,
    #[default]
    Active,
    NeedsAttention,
}

/// An icon as RGBA pixels (converted from the protocol's ARGB32 in network byte order).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrayIcon {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrayItem {
    /// The registered service string, used to address the item.
    pub service: String,
    pub bus_name: String,
    pub path: String,
    pub id: String,
    pub title: String,
    pub status: TrayStatus,
    /// Theme icon name (the attention icon while it needs attention).
    pub icon_name: String,
    /// Extra directory to look the icon up in.
    pub icon_theme_path: String,
    /// Pixel data when the application sends pixels instead of a name.
    pub icon: Option<TrayIcon>,
    pub tooltip: String,
    /// `com.canonical.dbusmenu` object, empty when there is none.
    pub menu_path: String,
    /// Clicking should open the menu rather than activate.
    pub item_is_menu: bool,
}

/// `":1.42/StatusNotifierItem"` or `"org.example.App"` → bus name and object path.
pub(super) fn split_service(service: &str) -> (String, String) {
    match service.find('/') {
        Some(i) => (service[..i].to_owned(), service[i..].to_owned()),
        None => (service.to_owned(), "/StatusNotifierItem".to_owned()),
    }
}

/// Picks the pixmap closest to 32px and converts ARGB (big endian) to RGBA.
fn best_pixmap(pixmaps: Vec<(i32, i32, Vec<u8>)>) -> Option<TrayIcon> {
    let (w, h, argb) = pixmaps
        .into_iter()
        .filter(|(w, h, data)| *w > 0 && *h > 0 && data.len() >= (*w * *h * 4) as usize)
        .min_by_key(|(w, _, _)| (w - 32).abs())?;
    let rgba = argb.as_chunks::<4>().0.iter().flat_map(|[a, r, g, b]| [*r, *g, *b, *a]).collect();
    Some(TrayIcon { width: w as u32, height: h as u32, rgba })
}

pub(super) async fn proxy<'a>(
    connection: &Connection,
    service: &str,
) -> zbus::Result<StatusNotifierItemProxy<'a>> {
    let (bus, path) = split_service(service);
    StatusNotifierItemProxy::builder(connection).destination(bus).path(path).build().await
}

pub(super) async fn read(
    connection: &Connection,
    service: &str,
    bus: &str,
    path: &str,
) -> zbus::Result<TrayItem> {
    let item = StatusNotifierItemProxy::builder(connection)
        .destination(bus.to_owned())
        .path(path.to_owned())
        // Always read fresh values: many items do not emit PropertiesChanged.
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await?;
    let status = match item.status().await.unwrap_or_default().as_str() {
        "Passive" => TrayStatus::Passive,
        "NeedsAttention" => TrayStatus::NeedsAttention,
        _ => TrayStatus::Active,
    };
    let attention = item.attention_icon_name().await.unwrap_or_default();
    let icon_name = match status {
        TrayStatus::NeedsAttention if !attention.is_empty() => attention,
        _ => item.icon_name().await.unwrap_or_default(),
    };
    let icon = if icon_name.is_empty() {
        item.icon_pixmap().await.ok().and_then(best_pixmap)
    } else {
        None
    };
    let tooltip = item
        .tool_tip()
        .await
        .map(|(_, _, title, body)| if body.is_empty() { title } else { format!("{title}\n{body}") })
        .unwrap_or_default();
    let menu_path = item.menu().await.map(|p| p.to_string()).unwrap_or_default();
    Ok(TrayItem {
        service: service.to_owned(),
        bus_name: bus.to_owned(),
        path: path.to_owned(),
        id: item.id().await.unwrap_or_default(),
        title: item.title().await.unwrap_or_default(),
        status,
        icon_name,
        icon_theme_path: item.icon_theme_path().await.unwrap_or_default(),
        icon,
        tooltip,
        menu_path: if menu_path == "/" { String::new() } else { menu_path },
        item_is_menu: item.item_is_menu().await.unwrap_or(false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_service_strings() {
        assert_eq!(
            split_service(":1.42/StatusNotifierItem"),
            (":1.42".into(), "/StatusNotifierItem".into())
        );
        assert_eq!(
            split_service(":1.24/org/ayatana/NotificationItem/nm_applet"),
            (":1.24".into(), "/org/ayatana/NotificationItem/nm_applet".into())
        );
        assert_eq!(
            split_service("com.example.App"),
            ("com.example.App".into(), "/StatusNotifierItem".into())
        );
    }

    #[test]
    fn converts_argb_and_picks_32px() {
        let small = (16, 16, [255u8, 1, 2, 3].repeat(256));
        let medium = (32, 32, [128u8, 10, 20, 30].repeat(1024));
        let broken = (64, 64, vec![0u8; 4]);
        let icon = best_pixmap(vec![small, medium, broken]).unwrap();
        assert_eq!((icon.width, icon.height), (32, 32));
        assert_eq!(&icon.rgba[..4], &[10, 20, 30, 128]);
    }
}
