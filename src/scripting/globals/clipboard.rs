//! The `ClipboardHistory` global.

use std::rc::Rc;

use slint_interpreter::Value;
use slintlu::clipboard;

use crate::scripting::bridge::{list_value, number_arg, string, struct_value};
use crate::scripting::{ShellHost, host};

const PREVIEW: usize = 80;

fn preview(text: &str) -> String {
    let line = text.trim().lines().next().unwrap_or_default();
    match line.char_indices().nth(PREVIEW) {
        Some((cut, _)) => format!("{}…", &line[..cut]),
        None => line.to_owned(),
    }
}

fn publish(host: &ShellHost) {
    let entries = clipboard::history().into_iter().map(|e| {
        struct_value([
            ("id", Value::Number(e.id as f64)),
            ("preview", string(preview(&e.text))),
            ("text", string(&e.text)),
        ])
    });
    host.set_global("ClipboardHistory", "entries", list_value(entries.collect::<Vec<_>>()));
}

pub fn install(host: &Rc<ShellHost>) {
    host.set_global("ClipboardHistory", "available", Value::Bool(clipboard::history_available()));
    clipboard::on_history_changed(|| {
        host::with(|host| publish(host));
    });
    host.on_callback(
        "ClipboardHistory",
        "copy",
        Rc::new(|args| {
            if let Some(id) = number_arg(args, 0) {
                clipboard::copy_from_history(id as u64);
            }
            Value::Void
        }),
    );
    host.on_callback(
        "ClipboardHistory",
        "remove",
        Rc::new(|args| {
            if let Some(id) = number_arg(args, 0) {
                clipboard::remove_from_history(id as u64);
            }
            Value::Void
        }),
    );
    host.on_callback(
        "ClipboardHistory",
        "clear",
        Rc::new(|_| {
            clipboard::clear_history();
            Value::Void
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previews_first_line() {
        assert_eq!(preview("  hello\nworld"), "hello");
        assert_eq!(preview(&"é".repeat(100)).chars().count(), PREVIEW + 1);
    }
}
