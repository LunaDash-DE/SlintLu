//! Values and callbacks shared by every instance: the `Shell` global and the registry used by
//! services to publish data into Slint globals.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use slint::{ModelRc, SharedString, VecModel};
use slint_interpreter::{ComponentInstance, Struct, Value};
use slintlu::OutputInfo;

/// A native implementation of a Slint global callback.
pub type Handler = Rc<dyn Fn(&[Value]) -> Value>;

/// Global property values and callbacks applied to every (current and future) instance.
#[derive(Default)]
pub struct Bridge {
    properties: RefCell<BTreeMap<(String, String), Value>>,
    callbacks: RefCell<BTreeMap<(String, String), Handler>>,
}

impl Bridge {
    pub fn set_property(&self, global: &str, property: &str, value: Value) {
        self.properties.borrow_mut().insert((global.into(), property.into()), value);
    }

    pub fn set_callback(&self, global: &str, callback: &str, handler: Handler) {
        self.callbacks.borrow_mut().insert((global.into(), callback.into()), handler);
    }

    /// The native handler of a global callback, if one is registered.
    pub fn callback(&self, global: &str, callback: &str) -> Option<Handler> {
        self.callbacks.borrow().get(&(global.to_owned(), callback.to_owned())).cloned()
    }

    /// Applies everything registered so far to a new instance.
    pub fn apply(&self, instance: &ComponentInstance) {
        for ((global, property), value) in self.properties.borrow().iter() {
            apply_property(instance, global, property, value.clone());
        }
        for ((global, callback), handler) in self.callbacks.borrow().iter() {
            apply_callback(instance, global, callback, handler.clone());
        }
    }
}

pub fn apply_property(instance: &ComponentInstance, global: &str, property: &str, value: Value) {
    if let Err(e) = instance.set_global_property(global, property, value) {
        // Globals the shell does not use are not exported; that is expected.
        log::trace!("cannot set {global}.{property}: {e}");
    }
}

pub fn apply_callback(
    instance: &ComponentInstance,
    global: &str,
    callback: &str,
    handler: Handler,
) {
    if let Err(e) = instance.set_global_callback(global, callback, move |args| handler(args)) {
        log::trace!("cannot set {global}.{callback}: {e}");
    }
}

/// `Screen` struct for an output.
pub fn screen_value(output: &OutputInfo) -> Value {
    let fields: [(&str, Value); 8] = [
        ("id", Value::Number(output.id as f64)),
        ("name", Value::String(output.name.as_str().into())),
        ("description", Value::String(output.description.as_str().into())),
        ("x", Value::Number(output.x as f64)),
        ("y", Value::Number(output.y as f64)),
        ("width", Value::Number(output.width as f64)),
        ("height", Value::Number(output.height as f64)),
        ("scale", Value::Number(output.scale_factor as f64)),
    ];
    Value::Struct(fields.into_iter().map(|(k, v)| (k.to_string(), v)).collect::<Struct>())
}

/// `[Screen]` model for all outputs.
pub fn screens_value(outputs: &[OutputInfo]) -> Value {
    list_value(outputs.iter().map(screen_value))
}

/// A Slint array from values.
pub fn list_value(values: impl IntoIterator<Item = Value>) -> Value {
    Value::Model(ModelRc::new(VecModel::from(values.into_iter().collect::<Vec<_>>())))
}

/// A Slint struct from `(field, value)` pairs.
pub fn struct_value<'a>(fields: impl IntoIterator<Item = (&'a str, Value)>) -> Value {
    Value::Struct(fields.into_iter().map(|(k, v)| (k.to_string(), v)).collect::<Struct>())
}

/// A Slint string.
pub fn string(value: impl AsRef<str>) -> Value {
    Value::String(SharedString::from(value.as_ref()))
}

/// An argument of a callback as a number.
pub fn number_arg(args: &[Value], index: usize) -> Option<f64> {
    match args.get(index) {
        Some(Value::Number(n)) => Some(*n),
        _ => None,
    }
}

/// An argument of a callback as a bool.
pub fn bool_arg(args: &[Value], index: usize) -> Option<bool> {
    match args.get(index) {
        Some(Value::Bool(b)) => Some(*b),
        _ => None,
    }
}

/// An argument of a callback as a string.
pub fn string_arg(args: &[Value], index: usize) -> Option<String> {
    match args.get(index) {
        Some(Value::String(s)) => Some(s.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_struct_has_all_fields() {
        let output = OutputInfo {
            id: 3,
            name: "DP-1".into(),
            description: "Monitor".into(),
            make: String::new(),
            model: String::new(),
            x: 1920,
            y: 0,
            width: 2560,
            height: 1440,
            scale_factor: 2,
        };
        let Value::Struct(s) = screen_value(&output) else { panic!("not a struct") };
        assert_eq!(s.get_field("name"), Some(&Value::String("DP-1".into())));
        assert_eq!(s.get_field("x"), Some(&Value::Number(1920.0)));
        assert_eq!(s.get_field("scale"), Some(&Value::Number(2.0)));
        assert_eq!(s.iter().count(), 8);
    }

    #[test]
    fn callback_arguments() {
        let args = [Value::String("ls".into()), Value::Number(0.5)];
        assert_eq!(string_arg(&args, 0).as_deref(), Some("ls"));
        assert_eq!(string_arg(&args, 1), None);
        assert_eq!(string_arg(&args, 5), None);
    }
}
