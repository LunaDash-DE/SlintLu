//! Bluetooth adapters and devices through BlueZ (system bus).
//!
//! BlueZ is never started on demand: while `bluetoothd` is not running the state is empty and
//! the watcher waits for it to appear.

use std::collections::HashMap;

use futures_util::StreamExt;
use zbus::fdo::{DBusProxy, ObjectManagerProxy, PropertiesProxy};
use zbus::names::{BusName, InterfaceName};
use zbus::{Connection, MatchRule, MessageStream, OwnedObjectPath, OwnedVariant, Variant, proxy};

const BLUEZ: &str = "org.bluez";

#[proxy(interface = "org.bluez.Device1", default_service = "org.bluez")]
trait Device1 {
    fn connect(&self) -> zbus::Result<()>;
    fn disconnect(&self) -> zbus::Result<()>;
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BluetoothDevice {
    /// Object path, used to address commands.
    pub path: String,
    pub address: String,
    pub name: String,
    /// freedesktop icon name, e.g. `audio-headset`.
    pub icon: String,
    pub connected: bool,
    pub paired: bool,
    /// Battery level in 0..1, if the device reports it.
    pub battery: Option<f32>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BluetoothState {
    /// BlueZ is running and an adapter exists.
    pub available: bool,
    pub powered: bool,
    /// Paired and connected devices, connected first.
    pub devices: Vec<BluetoothDevice>,
    adapter: Option<String>,
}

type Properties = HashMap<String, OwnedVariant>;
type Interfaces = HashMap<String, Properties>;

fn string(props: &Properties, key: &str) -> String {
    props.get(key).and_then(|v| String::try_from(&**v).ok()).unwrap_or_default()
}

fn boolean(props: &Properties, key: &str) -> bool {
    props.get(key).and_then(|v| bool::try_from(&**v).ok()).unwrap_or(false)
}

fn build(objects: HashMap<OwnedObjectPath, Interfaces>) -> BluetoothState {
    let mut state = BluetoothState::default();
    let mut paths: Vec<_> = objects.keys().cloned().collect();
    paths.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    for path in paths {
        let interfaces = &objects[&path];
        if let Some(adapter) = interfaces.get("org.bluez.Adapter1")
            && state.adapter.is_none()
        {
            state.available = true;
            state.powered = boolean(adapter, "Powered");
            state.adapter = Some(path.to_string());
        }
        if let Some(device) = interfaces.get("org.bluez.Device1") {
            let paired = boolean(device, "Paired");
            let connected = boolean(device, "Connected");
            if !paired && !connected {
                continue;
            }
            let battery = interfaces.get("org.bluez.Battery1").and_then(|b| {
                match b.get("Percentage").map(|v| &**v) {
                    Some(Variant::U8(p)) => Some(f32::from(*p) / 100.0),
                    _ => None,
                }
            });
            let alias = string(device, "Alias");
            state.devices.push(BluetoothDevice {
                path: path.to_string(),
                address: string(device, "Address"),
                name: if alias.is_empty() { string(device, "Name") } else { alias },
                icon: string(device, "Icon"),
                connected,
                paired,
                battery,
            });
        }
    }
    state.devices.sort_by(|a, b| b.connected.cmp(&a.connected).then_with(|| a.name.cmp(&b.name)));
    state
}

#[derive(Clone)]
pub struct Bluetooth {
    connection: Connection,
}

impl Bluetooth {
    pub fn connect() -> zbus::Result<Self> {
        Ok(Self { connection: zbus::block_on(Connection::system())? })
    }

    async fn running(&self) -> zbus::Result<bool> {
        let dbus = DBusProxy::new(&self.connection).await?;
        Ok(dbus.name_has_owner(BusName::try_from(BLUEZ)?).await?)
    }

    async fn snapshot(&self) -> zbus::Result<BluetoothState> {
        // Never call into BlueZ when it is not running: that would start it.
        if !self.running().await? {
            return Ok(BluetoothState::default());
        }
        let manager = ObjectManagerProxy::builder(&self.connection)
            .destination(BLUEZ)
            .path("/")
            .build()
            .await?;
        let objects = manager.get_managed_objects().await?;
        let objects = objects
            .into_iter()
            .map(|(path, interfaces)| {
                let interfaces = interfaces
                    .into_iter()
                    .map(|(name, props)| (name.to_string(), props.into_iter().collect()))
                    .collect();
                (path, interfaces)
            })
            .collect();
        Ok(build(objects))
    }

    pub fn state(&self) -> zbus::Result<BluetoothState> {
        zbus::block_on(self.snapshot())
    }

    /// Blocks, calling `on_state` initially and after every BlueZ change (including BlueZ
    /// starting or stopping).
    pub fn watch(&self, mut on_state: impl FnMut(&BluetoothState)) -> zbus::Result<()> {
        zbus::block_on(async {
            let changes =
                MatchRule::builder().msg_type(zbus::message::Type::Signal).sender(BLUEZ).build()?;
            let owner = MatchRule::builder()
                .msg_type(zbus::message::Type::Signal)
                .sender("org.freedesktop.DBus")
                .member("NameOwnerChanged")
                .arg(0, BLUEZ)
                .build()?;
            let mut changes =
                MessageStream::for_match_rule(changes, &self.connection, None).await?;
            let mut owner = MessageStream::for_match_rule(owner, &self.connection, None).await?;
            let mut last = self.snapshot().await?;
            on_state(&last);
            loop {
                futures_util::select! {
                    m = changes.next() => if m.is_none() { break },
                    m = owner.next() => if m.is_none() { break },
                }
                let state = self.snapshot().await?;
                if state != last {
                    on_state(&state);
                    last = state;
                }
            }
            Err(zbus::Error::Failure("the system bus closed".into()))
        })
    }

    pub fn set_powered(&self, powered: bool) -> zbus::Result<()> {
        zbus::block_on(async {
            let adapter = self
                .snapshot()
                .await?
                .adapter
                .ok_or_else(|| zbus::Error::Failure("no Bluetooth adapter".into()))?;
            let properties = PropertiesProxy::builder(&self.connection)
                .destination(BLUEZ)
                .path(adapter)
                .build()
                .await?;
            properties
                .set(
                    InterfaceName::try_from("org.bluez.Adapter1")?,
                    "Powered",
                    &Variant::from(powered),
                )
                .await
                .map_err(Into::into)
        })
    }

    /// Connects (`true`) or disconnects a device by its object path.
    pub fn set_connected(&self, path: &str, connected: bool) -> zbus::Result<()> {
        zbus::block_on(async {
            let device =
                Device1Proxy::builder(&self.connection).path(path.to_owned()).build().await?;
            if connected { device.connect().await } else { device.disconnect().await }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(pairs: Vec<(&str, OwnedVariant)>) -> Properties {
        pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
    }

    #[test]
    fn builds_state_from_managed_objects() {
        let mut objects: HashMap<OwnedObjectPath, Interfaces> = HashMap::new();
        objects.insert(
            OwnedObjectPath::try_from("/org/bluez/hci0").unwrap(),
            HashMap::from([(
                "org.bluez.Adapter1".to_string(),
                props(vec![("Powered", OwnedVariant::from(true))]),
            )]),
        );
        let device = |name: &str, connected: bool, paired: bool| {
            props(vec![
                ("Alias", OwnedVariant::from(zbus::Str::from(name.to_owned()))),
                ("Connected", OwnedVariant::from(connected)),
                ("Paired", OwnedVariant::from(paired)),
            ])
        };
        objects.insert(
            OwnedObjectPath::try_from("/org/bluez/hci0/dev_A").unwrap(),
            HashMap::from([("org.bluez.Device1".to_string(), device("Keyboard", false, true))]),
        );
        objects.insert(
            OwnedObjectPath::try_from("/org/bluez/hci0/dev_B").unwrap(),
            HashMap::from([
                ("org.bluez.Device1".to_string(), device("Headphones", true, true)),
                (
                    "org.bluez.Battery1".to_string(),
                    props(vec![("Percentage", OwnedVariant::from(80u8))]),
                ),
            ]),
        );
        objects.insert(
            OwnedObjectPath::try_from("/org/bluez/hci0/dev_C").unwrap(),
            HashMap::from([("org.bluez.Device1".to_string(), device("Stranger", false, false))]),
        );

        let state = build(objects);
        assert!(state.available && state.powered);
        assert_eq!(state.devices.len(), 2);
        assert_eq!(state.devices[0].name, "Headphones");
        assert_eq!(state.devices[0].battery, Some(0.8));
        assert_eq!(state.devices[1].name, "Keyboard");
    }
}
