//! Network status from NetworkManager (system bus).

use futures_util::StreamExt;
use zbus::{Connection, MatchRule, MessageStream, OwnedObjectPath, proxy};

const NM: &str = "org.freedesktop.NetworkManager";

#[proxy(
    interface = "org.freedesktop.NetworkManager",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager"
)]
trait NetworkManager {
    #[zbus(property)]
    fn state(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn connectivity(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn primary_connection(&self) -> zbus::Result<OwnedObjectPath>;
    #[zbus(property)]
    fn wireless_enabled(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn set_wireless_enabled(&self, enabled: bool) -> zbus::Result<()>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.Connection.Active",
    default_service = "org.freedesktop.NetworkManager"
)]
trait ActiveConnection {
    #[zbus(property)]
    fn id(&self) -> zbus::Result<String>;
    #[zbus(property, name = "Type")]
    fn kind(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn specific_object(&self) -> zbus::Result<OwnedObjectPath>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.AccessPoint",
    default_service = "org.freedesktop.NetworkManager"
)]
trait AccessPoint {
    #[zbus(property)]
    fn ssid(&self) -> zbus::Result<Vec<u8>>;
    #[zbus(property)]
    fn strength(&self) -> zbus::Result<u8>;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ConnectionKind {
    #[default]
    None,
    Wifi,
    Ethernet,
    Vpn,
    Other,
}

impl ConnectionKind {
    fn from_nm(kind: &str) -> Self {
        match kind {
            "802-11-wireless" => Self::Wifi,
            "802-3-ethernet" => Self::Ethernet,
            "vpn" | "wireguard" => Self::Vpn,
            "" => Self::None,
            _ => Self::Other,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Wifi => "wifi",
            Self::Ethernet => "ethernet",
            Self::Vpn => "vpn",
            Self::Other => "other",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NetworkState {
    /// Connected with full internet connectivity.
    pub online: bool,
    /// Connected to some network (possibly without internet).
    pub connected: bool,
    pub kind: ConnectionKind,
    /// Connection name or Wi-Fi SSID.
    pub name: String,
    /// Wi-Fi signal strength in 0..1.
    pub strength: f32,
    pub wifi_enabled: bool,
}

/// NM_STATE_CONNECTED_LOCAL and above.
const CONNECTED_LOCAL: u32 = 50;
/// NM_CONNECTIVITY_FULL.
const CONNECTIVITY_FULL: u32 = 4;

async fn snapshot(connection: &Connection) -> zbus::Result<NetworkState> {
    let nm = NetworkManagerProxy::new(connection).await?;
    let state = nm.state().await?;
    let mut result = NetworkState {
        connected: state >= CONNECTED_LOCAL,
        online: nm.connectivity().await.unwrap_or(0) == CONNECTIVITY_FULL,
        wifi_enabled: nm.wireless_enabled().await.unwrap_or(false),
        ..Default::default()
    };
    let primary = nm.primary_connection().await?;
    if primary.as_str() == "/" {
        return Ok(result);
    }
    let active = ActiveConnectionProxy::builder(connection).path(primary).build().await?;
    result.kind = ConnectionKind::from_nm(&active.kind().await.unwrap_or_default());
    result.name = active.id().await.unwrap_or_default();
    if result.kind == ConnectionKind::Wifi
        && let Ok(ap_path) = active.specific_object().await
        && ap_path.as_str() != "/"
    {
        let ap = AccessPointProxy::builder(connection).path(ap_path).build().await?;
        if let Ok(ssid) = ap.ssid().await {
            result.name = String::from_utf8_lossy(&ssid).into_owned();
        }
        result.strength = f32::from(ap.strength().await.unwrap_or(0)) / 100.0;
    }
    Ok(result)
}

#[derive(Clone)]
pub struct NetworkMonitor {
    connection: Connection,
}

impl NetworkMonitor {
    pub fn connect() -> zbus::Result<Self> {
        let connection = zbus::block_on(Connection::system())?;
        // Fail early when NetworkManager is not installed.
        zbus::block_on(NetworkManagerProxy::new(&connection))?;
        Ok(Self { connection })
    }

    pub fn state(&self) -> zbus::Result<NetworkState> {
        zbus::block_on(snapshot(&self.connection))
    }

    /// Blocks, calling `on_state` initially and after every NetworkManager change.
    pub fn watch(&self, mut on_state: impl FnMut(&NetworkState)) -> zbus::Result<()> {
        zbus::block_on(async {
            let rule = MatchRule::builder()
                .msg_type(zbus::message::Type::Signal)
                .sender(NM)
                .interface("org.freedesktop.DBus.Properties")
                .member("PropertiesChanged")
                .build()?;
            let mut changes = MessageStream::for_match_rule(rule, &self.connection, None).await?;
            let mut last = snapshot(&self.connection).await?;
            on_state(&last);
            while changes.next().await.is_some() {
                let state = snapshot(&self.connection).await?;
                if state != last {
                    on_state(&state);
                    last = state;
                }
            }
            Err(zbus::Error::Failure("the system bus closed".into()))
        })
    }

    pub fn set_wifi_enabled(&self, enabled: bool) -> zbus::Result<()> {
        zbus::block_on(async {
            NetworkManagerProxy::new(&self.connection).await?.set_wireless_enabled(enabled).await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_kinds() {
        assert_eq!(ConnectionKind::from_nm("802-11-wireless"), ConnectionKind::Wifi);
        assert_eq!(ConnectionKind::from_nm("802-3-ethernet"), ConnectionKind::Ethernet);
        assert_eq!(ConnectionKind::from_nm("wireguard"), ConnectionKind::Vpn);
        assert_eq!(ConnectionKind::from_nm("bluetooth"), ConnectionKind::Other);
        assert_eq!(ConnectionKind::from_nm("").as_str(), "none");
    }
}
