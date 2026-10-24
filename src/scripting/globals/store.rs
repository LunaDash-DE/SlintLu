//! The `Store` global: persistent settings per shell.

use std::cell::RefCell;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;

use serde_json::{Value as Json, json};
use slint_interpreter::Value;
use slintlu::store::Store;
use slintlu::util::xdg;

use crate::scripting::ShellHost;
use crate::scripting::bridge::{bool_arg, number_arg, string, string_arg};

thread_local! {
    static STORE: RefCell<Store> = RefCell::default();
    static REVISION: RefCell<u32> = const { RefCell::new(0) };
}

fn bump(host: &ShellHost) {
    let revision = REVISION.with(|r| {
        *r.borrow_mut() += 1;
        *r.borrow()
    });
    host.set_global("Store", "revision", Value::Number(revision.into()));
}

fn setter(host: &Rc<ShellHost>, name: &str, convert: fn(&[Value]) -> Option<Json>) {
    let weak = Rc::downgrade(host);
    host.on_callback(
        "Store",
        name,
        Rc::new(move |args| {
            if let (Some(key), Some(value)) = (string_arg(args, 0), convert(args)) {
                STORE.with(|s| s.borrow_mut().set(&key, value));
                if let Some(host) = weak.upgrade() {
                    bump(&host);
                }
            }
            Value::Void
        }),
    );
}

pub fn install(host: &Rc<ShellHost>) {
    let mut hasher = DefaultHasher::new();
    host.config().hash(&mut hasher);
    let path =
        xdg::state_home().join("slintlu").join(format!("store-{:016x}.json", hasher.finish()));
    STORE.with(|s| *s.borrow_mut() = Store::load(&path));
    log::debug!("store: {}", path.display());

    host.on_callback(
        "Store",
        "get-string",
        Rc::new(|args| {
            let key = string_arg(args, 0).unwrap_or_default();
            let fallback = string_arg(args, 1).unwrap_or_default();
            let value =
                STORE.with(|s| s.borrow().get(&key).and_then(Json::as_str).map(str::to_owned));
            string(value.unwrap_or(fallback))
        }),
    );
    host.on_callback(
        "Store",
        "get-number",
        Rc::new(|args| {
            let key = string_arg(args, 0).unwrap_or_default();
            let fallback = number_arg(args, 1).unwrap_or(0.0);
            Value::Number(
                STORE.with(|s| s.borrow().get(&key).and_then(Json::as_f64)).unwrap_or(fallback),
            )
        }),
    );
    host.on_callback(
        "Store",
        "get-bool",
        Rc::new(|args| {
            let key = string_arg(args, 0).unwrap_or_default();
            let fallback = bool_arg(args, 1).unwrap_or(false);
            Value::Bool(
                STORE.with(|s| s.borrow().get(&key).and_then(Json::as_bool)).unwrap_or(fallback),
            )
        }),
    );
    setter(host, "set-string", |a| string_arg(a, 1).map(Json::String));
    setter(host, "set-number", |a| number_arg(a, 1).map(|n| json!(n)));
    setter(host, "set-bool", |a| bool_arg(a, 1).map(Json::Bool));
    let weak = Rc::downgrade(host);
    host.on_callback(
        "Store",
        "remove",
        Rc::new(move |args| {
            if let Some(key) = string_arg(args, 0) {
                STORE.with(|s| s.borrow_mut().remove(&key));
                if let Some(host) = weak.upgrade() {
                    bump(&host);
                }
            }
            Value::Void
        }),
    );
}
