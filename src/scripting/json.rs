//! Conversion between Slint interpreter values and JSON, for IPC.

use serde_json::{Map, Number, Value as Json};
use slint::Model;
use slint_interpreter::{Struct, Value};

use super::bridge::list_value;

pub fn to_json(value: &Value) -> Json {
    match value {
        Value::Void => Json::Null,
        Value::Number(n) => Number::from_f64(*n).map_or(Json::Null, |n| {
            // Integral values print as integers (`3`, not `3.0`).
            if n.as_f64().is_some_and(|f| f.fract() == 0.0 && f.abs() < 9.0e15) {
                Json::from(n.as_f64().unwrap_or_default() as i64)
            } else {
                Json::Number(n)
            }
        }),
        Value::String(s) => Json::String(s.to_string()),
        Value::Bool(b) => Json::Bool(*b),
        Value::EnumerationValue(_, v) => Json::String(v.clone()),
        Value::Brush(slint::Brush::SolidColor(c)) => Json::String(format!(
            "#{:02x}{:02x}{:02x}{:02x}",
            c.red(),
            c.green(),
            c.blue(),
            c.alpha()
        )),
        Value::Model(model) => Json::Array(model.iter().map(|v| to_json(&v)).collect()),
        Value::Struct(s) => {
            Json::Object(s.iter().map(|(k, v)| (k.to_owned(), to_json(v))).collect::<Map<_, _>>())
        }
        // Images are reported by the file they came from; pixel data would be huge.
        Value::Image(image) => {
            image.path().map_or(Json::Null, |p| Json::String(p.display().to_string()))
        }
        Value::Brush(_) => Json::String("gradient".into()),
        _ => Json::Null,
    }
}

pub fn from_json(json: &Json) -> Value {
    match json {
        Json::Null => Value::Void,
        Json::Bool(b) => Value::Bool(*b),
        Json::Number(n) => Value::Number(n.as_f64().unwrap_or(0.0)),
        Json::String(s) => Value::String(s.as_str().into()),
        Json::Array(items) => list_value(items.iter().map(from_json)),
        Json::Object(fields) => {
            Value::Struct(fields.iter().map(|(k, v)| (k.clone(), from_json(v))).collect::<Struct>())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn round_trips_plain_values() {
        for json in [
            json!(null),
            json!(true),
            json!(3),
            json!(0.5),
            json!("hi"),
            json!([1, "a"]),
            json!({"x": 1}),
        ] {
            assert_eq!(to_json(&from_json(&json)), json, "{json}");
        }
    }

    #[test]
    fn colours_become_hex() {
        let value = Value::Brush(slint::Color::from_rgb_u8(0x89, 0xb4, 0xfa).into());
        assert_eq!(to_json(&value), json!("#89b4faff"));
    }

    #[test]
    fn enums_become_strings() {
        let value = Value::EnumerationValue("Layer".into(), "top".into());
        assert_eq!(to_json(&value), json!("top"));
    }
}
