//! Audio device description.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceKind {
    /// Speakers, headphones (a sink).
    Output,
    /// Microphones (a source).
    Input,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Device {
    /// Server-side name, used to select the device.
    pub name: String,
    /// Human readable name.
    pub description: String,
    /// Average channel volume; 1.0 is 100 %, values above amplify.
    pub volume: f32,
    pub muted: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AudioState {
    pub outputs: Vec<Device>,
    pub inputs: Vec<Device>,
    pub default_output: Option<String>,
    pub default_input: Option<String>,
}

impl AudioState {
    fn find<'a>(devices: &'a [Device], name: Option<&String>) -> Option<&'a Device> {
        name.and_then(|n| devices.iter().find(|d| &d.name == n)).or_else(|| devices.first())
    }

    /// The default output, or the first one.
    pub fn output(&self) -> Option<&Device> {
        Self::find(&self.outputs, self.default_output.as_ref())
    }

    /// The default input, or the first one.
    pub fn input(&self) -> Option<&Device> {
        Self::find(&self.inputs, self.default_input.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_device_falls_back_to_first() {
        let device = |name: &str| Device { name: name.into(), ..Default::default() };
        let mut state = AudioState {
            outputs: vec![device("a"), device("b")],
            default_output: Some("b".into()),
            ..Default::default()
        };
        assert_eq!(state.output().unwrap().name, "b");
        state.default_output = Some("gone".into());
        assert_eq!(state.output().unwrap().name, "a");
        assert!(state.input().is_none());
    }
}
