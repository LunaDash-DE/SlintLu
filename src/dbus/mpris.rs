//! Media players through MPRIS (`org.mpris.MediaPlayer2.*` on the session bus).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt;
use zbus::{Connection, MatchRule, MessageStream, OwnedVariant, Variant, proxy};

const PREFIX: &str = "org.mpris.MediaPlayer2.";
/// How often the position of a playing track is refreshed (players do not signal it).
const POSITION_POLL: Duration = Duration::from_secs(1);

#[proxy(interface = "org.mpris.MediaPlayer2", default_path = "/org/mpris/MediaPlayer2")]
trait MediaPlayer2 {
    #[zbus(property)]
    fn identity(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn desktop_entry(&self) -> zbus::Result<String>;
}

#[proxy(interface = "org.mpris.MediaPlayer2.Player", default_path = "/org/mpris/MediaPlayer2")]
trait Player {
    fn play_pause(&self) -> zbus::Result<()>;
    fn next(&self) -> zbus::Result<()>;
    fn previous(&self) -> zbus::Result<()>;
    fn seek(&self, offset: i64) -> zbus::Result<()>;
    #[zbus(property)]
    fn playback_status(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn metadata(&self) -> zbus::Result<HashMap<String, OwnedVariant>>;
    #[zbus(property)]
    fn position(&self) -> zbus::Result<i64>;
    #[zbus(property)]
    fn can_go_next(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn can_go_previous(&self) -> zbus::Result<bool>;
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MediaPlayer {
    /// Bus name, used to address commands.
    pub bus_name: String,
    /// e.g. "Spotify" or "Mozilla Firefox".
    pub identity: String,
    pub desktop_entry: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    /// `file://` or `https://` URL of the cover art.
    pub art_url: String,
    pub playing: bool,
    pub paused: bool,
    /// Seconds.
    pub position: f64,
    pub length: f64,
    pub can_go_next: bool,
    pub can_go_previous: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MediaState {
    pub players: Vec<MediaPlayer>,
}

impl MediaState {
    /// The player a shell should show: the first playing one, else the first paused one, else
    /// any.
    pub fn active(&self) -> Option<&MediaPlayer> {
        self.players
            .iter()
            .find(|p| p.playing)
            .or_else(|| self.players.iter().find(|p| p.paused))
            .or_else(|| self.players.first())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaCommand {
    PlayPause,
    Next,
    Previous,
    /// Relative seek in seconds.
    Seek(i64),
}

fn metadata_str(metadata: &HashMap<String, OwnedVariant>, key: &str) -> String {
    match metadata.get(key).map(|v| &**v) {
        Some(Variant::Str(s)) => s.to_string(),
        Some(Variant::ObjectPath(p)) => p.to_string(),
        // xesam:artist is a list of strings.
        Some(Variant::Array(items)) => {
            items.iter().filter_map(|v| String::try_from(v).ok()).collect::<Vec<_>>().join(", ")
        }
        _ => String::new(),
    }
}

fn metadata_micros(metadata: &HashMap<String, OwnedVariant>, key: &str) -> f64 {
    match metadata.get(key).map(|v| &**v) {
        Some(Variant::I64(v)) => *v as f64 / 1e6,
        Some(Variant::U64(v)) => *v as f64 / 1e6,
        Some(Variant::I32(v)) => *v as f64 / 1e6,
        _ => 0.0,
    }
}

async fn read_player(connection: &Connection, bus_name: &str) -> zbus::Result<MediaPlayer> {
    let root = MediaPlayer2Proxy::builder(connection).destination(bus_name).build().await?;
    let player = PlayerProxy::builder(connection).destination(bus_name).build().await?;
    let metadata = player.metadata().await.unwrap_or_default();
    let status = player.playback_status().await.unwrap_or_default();
    Ok(MediaPlayer {
        bus_name: bus_name.to_owned(),
        identity: root
            .identity()
            .await
            .unwrap_or_else(|_| bus_name.trim_start_matches(PREFIX).to_owned()),
        desktop_entry: root.desktop_entry().await.unwrap_or_default(),
        title: metadata_str(&metadata, "xesam:title"),
        artist: metadata_str(&metadata, "xesam:artist"),
        album: metadata_str(&metadata, "xesam:album"),
        art_url: metadata_str(&metadata, "mpris:artUrl"),
        playing: status == "Playing",
        paused: status == "Paused",
        position: player.position().await.map(|p| p as f64 / 1e6).unwrap_or(0.0),
        length: metadata_micros(&metadata, "mpris:length"),
        can_go_next: player.can_go_next().await.unwrap_or(false),
        can_go_previous: player.can_go_previous().await.unwrap_or(false),
    })
}

async fn snapshot(connection: &Connection) -> zbus::Result<MediaState> {
    let dbus = zbus::fdo::DBusProxy::new(connection).await?;
    let mut names: Vec<String> = dbus
        .list_names()
        .await?
        .into_iter()
        .map(|n| n.to_string())
        .filter(|n| n.starts_with(PREFIX))
        .collect();
    names.sort();
    let mut players = Vec::new();
    for name in names {
        match read_player(connection, &name).await {
            Ok(player) => players.push(player),
            Err(e) => log::debug!("mpris: cannot read {name}: {e}"),
        }
    }
    Ok(MediaState { players })
}

/// Connection to the session bus for media players.
#[derive(Clone)]
pub struct Mpris {
    connection: Connection,
}

impl Mpris {
    pub fn connect() -> zbus::Result<Self> {
        Ok(Self { connection: zbus::block_on(Connection::session())? })
    }

    pub fn state(&self) -> zbus::Result<MediaState> {
        zbus::block_on(snapshot(&self.connection))
    }

    /// Blocks, calling `on_state` initially and whenever players appear, disappear or change.
    /// While something plays, the state is also refreshed every second for the position.
    pub fn watch(&self, on_state: impl FnMut(&MediaState) + Send + 'static) -> zbus::Result<()> {
        let last = Arc::new(Mutex::new(None::<MediaState>));
        let on_state = Arc::new(Mutex::new(on_state));
        let publish = {
            let (last, on_state) = (last.clone(), on_state.clone());
            move |state: MediaState| {
                let mut last = last.lock().unwrap_or_else(|e| e.into_inner());
                if last.as_ref() != Some(&state) {
                    (on_state.lock().unwrap_or_else(|e| e.into_inner()))(&state);
                    *last = Some(state);
                }
            }
        };

        let poller = self.clone();
        let poll_publish = publish.clone();
        let poll_last = last.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(POSITION_POLL);
                let playing = poll_last
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .as_ref()
                    .is_some_and(|s| s.players.iter().any(|p| p.playing));
                if playing && let Ok(state) = poller.state() {
                    poll_publish(state);
                }
            }
        });

        publish(self.state()?);
        zbus::block_on(async {
            let owners = MatchRule::builder()
                .msg_type(zbus::message::Type::Signal)
                .sender("org.freedesktop.DBus")
                .member("NameOwnerChanged")
                .arg0ns("org.mpris.MediaPlayer2")
                .build()?;
            let properties = MatchRule::builder()
                .msg_type(zbus::message::Type::Signal)
                .interface("org.freedesktop.DBus.Properties")
                .member("PropertiesChanged")
                .path("/org/mpris/MediaPlayer2")
                .build()?;
            let mut owners = MessageStream::for_match_rule(owners, &self.connection, None).await?;
            let mut properties =
                MessageStream::for_match_rule(properties, &self.connection, None).await?;
            loop {
                futures_util::select! {
                    m = owners.next() => if m.is_none() { break },
                    m = properties.next() => if m.is_none() { break },
                }
                publish(snapshot(&self.connection).await?);
            }
            Err(zbus::Error::Failure("the session bus closed".into()))
        })
    }

    /// Sends a command to the player with the given bus name.
    pub fn command(&self, bus_name: &str, command: MediaCommand) -> zbus::Result<()> {
        zbus::block_on(async {
            let player =
                PlayerProxy::builder(&self.connection).destination(bus_name).build().await?;
            match command {
                MediaCommand::PlayPause => player.play_pause().await,
                MediaCommand::Next => player.next().await,
                MediaCommand::Previous => player.previous().await,
                MediaCommand::Seek(seconds) => player.seek(seconds * 1_000_000).await,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_player_prefers_playing() {
        let player = |name: &str, playing, paused| MediaPlayer {
            bus_name: name.into(),
            playing,
            paused,
            ..Default::default()
        };
        let state = MediaState {
            players: vec![
                player("a", false, false),
                player("b", false, true),
                player("c", true, false),
            ],
        };
        assert_eq!(state.active().unwrap().bus_name, "c");
        let state =
            MediaState { players: vec![player("a", false, false), player("b", false, true)] };
        assert_eq!(state.active().unwrap().bus_name, "b");
        assert!(MediaState::default().active().is_none());
    }

    #[test]
    fn metadata_lists_are_joined() {
        let mut metadata = HashMap::new();
        metadata.insert(
            "xesam:artist".to_string(),
            OwnedVariant::try_from(Variant::from(vec!["A", "B"])).unwrap(),
        );
        metadata.insert("xesam:title".to_string(), OwnedVariant::from(zbus::Str::from("Song")));
        metadata.insert("mpris:length".to_string(), OwnedVariant::from(2_500_000i64));
        assert_eq!(metadata_str(&metadata, "xesam:artist"), "A, B");
        assert_eq!(metadata_str(&metadata, "xesam:title"), "Song");
        assert_eq!(metadata_micros(&metadata, "mpris:length"), 2.5);
    }
}
