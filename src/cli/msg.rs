//! `slintlu msg`: send a command to the running shell.

use clap::Subcommand;
use serde_json::Value;
use slintlu::ipc::{Client, Request, socket_path};

#[derive(Subcommand, Debug)]
pub enum MsgCommand {
    /// Check that the shell is running.
    Ping,
    /// Reload the configuration.
    Reload,
    /// Exit the shell.
    Quit,
    /// List the shell's windows.
    Windows,
    /// List the monitors.
    Outputs,
    /// Print `Global.property`.
    Get { target: String },
    /// Set `Global.property` (values are JSON: true, 0.5, "text"; bare words are strings).
    Set { target: String, value: String },
    /// Flip the boolean `Global.property`.
    Toggle { target: String },
    /// Invoke `Global.callback` with arguments.
    Call { target: String, args: Vec<String> },
}

/// Command line values are JSON when they parse as such, plain strings otherwise.
fn value(arg: &str) -> Value {
    serde_json::from_str(arg).unwrap_or_else(|_| Value::String(arg.to_owned()))
}

impl From<MsgCommand> for Request {
    fn from(command: MsgCommand) -> Self {
        match command {
            MsgCommand::Ping => Request::Ping,
            MsgCommand::Reload => Request::Reload,
            MsgCommand::Quit => Request::Quit,
            MsgCommand::Windows => Request::Windows,
            MsgCommand::Outputs => Request::Outputs,
            MsgCommand::Get { target } => Request::Get { target },
            MsgCommand::Set { target, value: v } => Request::Set { target, value: value(&v) },
            MsgCommand::Toggle { target } => Request::Toggle { target },
            MsgCommand::Call { target, args } => {
                Request::Call { target, args: args.iter().map(|a| value(a)).collect() }
            }
        }
    }
}

pub fn msg(command: MsgCommand) -> Result<(), String> {
    let mut client = Client::connect(&socket_path()).map_err(|e| e.to_string())?;
    let response = client.send(&command.into()).map_err(|e| e.to_string())?;
    if !response.ok {
        return Err(response.error.unwrap_or_else(|| "request failed".into()));
    }
    match response.result {
        Value::Null => {}
        Value::String(s) => println!("{s}"),
        other => println!("{}", serde_json::to_string_pretty(&other).unwrap_or_default()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn arguments_are_json_or_strings() {
        assert_eq!(value("0.5"), json!(0.5));
        assert_eq!(value("true"), json!(true));
        assert_eq!(value("\"quoted\""), json!("quoted"));
        assert_eq!(value("firefox"), json!("firefox"));
    }
}
