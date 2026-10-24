//! Wire types.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A request to the running shell.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "kebab-case")]
pub enum Request {
    /// Health check; answers with the version.
    Ping,
    /// Recompile and reload the configuration.
    Reload,
    /// Exit the shell.
    Quit,
    /// List the shell's windows.
    Windows,
    /// List the monitors.
    Outputs,
    /// Read `Global.property`.
    Get { target: String },
    /// Write `Global.property`.
    Set { target: String, value: Value },
    /// Flip a boolean `Global.property`.
    Toggle { target: String },
    /// Invoke `Global.callback` or `Global.function` with arguments.
    Call {
        target: String,
        #[serde(default)]
        args: Vec<Value>,
    },
}

/// The answer to a [`Request`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub result: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Response {
    pub fn success(result: Value) -> Self {
        Self { ok: true, result, error: None }
    }

    pub fn failure(error: impl Into<String>) -> Self {
        Self { ok: false, result: Value::Null, error: Some(error.into()) }
    }
}

/// `Global.member`, as used by `get`, `set`, `toggle` and `call`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target<'a> {
    pub global: &'a str,
    pub member: &'a str,
}

impl<'a> Target<'a> {
    pub fn parse(target: &'a str) -> Result<Self, String> {
        match target.split_once('.') {
            Some((global, member)) if !global.is_empty() && !member.is_empty() => {
                Ok(Self { global, member })
            }
            _ => Err(format!("`{target}` is not of the form Global.member")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn requests_use_a_cmd_tag() {
        let request = Request::Call { target: "Audio.set-volume".into(), args: vec![json!(0.5)] };
        let text = serde_json::to_string(&request).unwrap();
        assert_eq!(text, r#"{"cmd":"call","target":"Audio.set-volume","args":[0.5]}"#);
        assert_eq!(serde_json::from_str::<Request>(&text).unwrap(), request);
        assert_eq!(serde_json::from_str::<Request>(r#"{"cmd":"ping"}"#).unwrap(), Request::Ping);
        assert_eq!(
            serde_json::from_str::<Request>(r#"{"cmd":"call","target":"A.b"}"#).unwrap(),
            Request::Call { target: "A.b".into(), args: vec![] }
        );
    }

    #[test]
    fn responses_omit_empty_fields() {
        assert_eq!(
            serde_json::to_string(&Response::success(Value::Null)).unwrap(),
            r#"{"ok":true}"#
        );
        assert_eq!(
            serde_json::to_string(&Response::failure("nope")).unwrap(),
            r#"{"ok":false,"error":"nope"}"#
        );
    }

    #[test]
    fn targets() {
        assert_eq!(
            Target::parse("Launcher.open").unwrap(),
            Target { global: "Launcher", member: "open" }
        );
        assert!(Target::parse("Launcher").is_err());
        assert!(Target::parse(".open").is_err());
    }
}
