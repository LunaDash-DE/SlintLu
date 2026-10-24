//! The `pactl` command line client: JSON listings, `subscribe` for change events.

use std::io::{self, BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

use super::model::{AudioState, Device, DeviceKind};

/// pactl reports volumes relative to this "100 %" value.
const NORMAL_VOLUME: f64 = 65536.0;
/// Coalesces the bursts of events a single volume change produces.
const EVENT_COALESCE: Duration = Duration::from_millis(30);

#[derive(Debug, Default, Clone, Copy)]
pub struct Pactl;

fn kind_target(kind: DeviceKind) -> (&'static str, &'static str) {
    match kind {
        DeviceKind::Output => ("sink", "@DEFAULT_SINK@"),
        DeviceKind::Input => ("source", "@DEFAULT_SOURCE@"),
    }
}

fn devices(listing: &Value, skip_monitors: bool) -> Vec<Device> {
    listing
        .as_array()
        .into_iter()
        .flatten()
        .filter(|d| {
            !skip_monitors || d["monitor_of_sink"].is_null() || d["monitor_of_sink"] == "n/a"
        })
        .filter_map(|d| {
            let channels: Vec<f64> = d["volume"]
                .as_object()
                .map(|c| c.values().filter_map(|v| v["value"].as_f64()).collect())
                .unwrap_or_default();
            let volume = if channels.is_empty() {
                0.0
            } else {
                channels.iter().sum::<f64>() / channels.len() as f64 / NORMAL_VOLUME
            };
            Some(Device {
                name: d["name"].as_str()?.to_owned(),
                description: d["description"].as_str().unwrap_or_default().to_owned(),
                volume: volume as f32,
                muted: d["mute"].as_bool().unwrap_or(false),
            })
        })
        .collect()
}

impl Pactl {
    /// Returns a client if `pactl` can reach a sound server.
    pub fn detect() -> Option<Self> {
        let ok = Command::new("pactl")
            .arg("info")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        ok.then_some(Self)
    }

    fn output(args: &[&str]) -> io::Result<String> {
        let out = Command::new("pactl").args(args).stderr(Stdio::null()).output()?;
        if !out.status.success() {
            return Err(io::Error::other(format!("pactl {} failed", args.join(" "))));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    fn run(args: &[&str]) -> io::Result<()> {
        Self::output(args).map(drop)
    }

    pub fn state(&self) -> io::Result<AudioState> {
        let list = |what: &str| -> io::Result<Value> {
            serde_json::from_str(&Self::output(&["-f", "json", "list", what])?)
                .map_err(io::Error::other)
        };
        let default = |what: &str| {
            Self::output(&[what]).ok().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
        };
        Ok(AudioState {
            outputs: devices(&list("sinks")?, false),
            inputs: devices(&list("sources")?, true),
            default_output: default("get-default-sink"),
            default_input: default("get-default-source"),
        })
    }

    /// Blocks, calling `on_state` initially and after every change on the sound server.
    pub fn watch(&self, on_state: &mut dyn FnMut(&AudioState)) -> io::Result<()> {
        let mut child = Command::new("pactl")
            .arg("subscribe")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let stdout = child.stdout.take().ok_or_else(|| io::Error::other("no pactl stdout"))?;
        let mut last = self.state()?;
        on_state(&last);
        let mut last_refresh = Instant::now();
        for line in BufReader::new(stdout).lines() {
            let line = line?;
            let relevant = ["sink", "source", "server", "card"]
                .iter()
                .any(|kind| line.contains(&format!(" on {kind}")))
                && !line.contains("sink-input")
                && !line.contains("source-output");
            if !relevant || last_refresh.elapsed() < EVENT_COALESCE {
                continue;
            }
            last_refresh = Instant::now();
            let state = self.state()?;
            if state != last {
                on_state(&state);
                last = state;
            }
        }
        let _ = child.kill();
        Err(io::Error::new(io::ErrorKind::UnexpectedEof, "pactl subscribe ended"))
    }

    /// Sets the default device's volume (1.0 = 100 %, clamped to 0..=1.5).
    pub fn set_volume(&self, kind: DeviceKind, volume: f32) -> io::Result<()> {
        let (what, target) = kind_target(kind);
        let percent = format!("{}%", (volume.clamp(0.0, 1.5) * 100.0).round());
        Self::run(&[&format!("set-{what}-volume"), target, &percent])
    }

    pub fn set_muted(&self, kind: DeviceKind, muted: bool) -> io::Result<()> {
        let (what, target) = kind_target(kind);
        Self::run(&[&format!("set-{what}-mute"), target, if muted { "1" } else { "0" }])
    }

    pub fn toggle_muted(&self, kind: DeviceKind) -> io::Result<()> {
        let (what, target) = kind_target(kind);
        Self::run(&[&format!("set-{what}-mute"), target, "toggle"])
    }

    pub fn set_default(&self, kind: DeviceKind, name: &str) -> io::Result<()> {
        let (what, _) = kind_target(kind);
        Self::run(&[&format!("set-default-{what}"), name])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_device_listings() {
        let listing = json!([
            {"name": "alsa_output.analog", "description": "Speakers", "mute": false,
             "volume": {"front-left": {"value": 55706}, "front-right": {"value": 55706}}},
            {"name": "alsa_output.analog.monitor", "description": "Monitor", "mute": true,
             "monitor_of_sink": "alsa_output.analog", "volume": {"mono": {"value": 65536}}}
        ]);
        let all = devices(&listing, false);
        assert_eq!(all.len(), 2);
        assert!((all[0].volume - 0.85).abs() < 0.001);
        assert!(all[1].muted);
        assert_eq!(devices(&listing, true).len(), 1);
    }
}
