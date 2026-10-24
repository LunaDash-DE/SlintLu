//! The `Store` (persistent settings) and `Shared` (cross-window state) globals.

use std::cell::RefCell;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;

use serde_json::{Value as Json, json};
use slint_interpreter::Value;
use slintlu::store::Store;
use slintlu::util::xdg;

use crate::scripting::ShellHost;
use crate::scripting::bridge::{bool_arg, number_arg, string, string_arg};

/// One key–value global: its Slint name, the values and a change counter.
struct KeyValues {
    global: &'static str,
    store: RefCell<Store>,
    revision: RefCell<u32>,
}

impl KeyValues {
    fn new(global: &'static str) -> Rc<Self> {
        Rc::new(Self { global, store: RefCell::default(), revision: RefCell::new(0) })
    }

    fn bump(&self, host: &ShellHost) {
        let revision = {
            let mut r = self.revision.borrow_mut();
            *r += 1;
            *r
        };
        host.set_global(self.global, "revision", Value::Number(revision.into()));
    }

    fn set(self: &Rc<Self>, host: &ShellHost, key: &str, value: Json) {
        self.store.borrow_mut().set(key, value);
        self.bump(host);
    }
}

fn getter(
    host: &Rc<ShellHost>,
    kv: &Rc<KeyValues>,
    name: &str,
    read: fn(&Store, &[Value]) -> Value,
) {
    let kv2 = kv.clone();
    host.on_callback(kv.global, name, Rc::new(move |args| read(&kv2.store.borrow(), args)));
}

fn setter(
    host: &Rc<ShellHost>,
    kv: &Rc<KeyValues>,
    name: &str,
    convert: fn(&[Value]) -> Option<Json>,
) {
    let weak = Rc::downgrade(host);
    let kv2 = kv.clone();
    host.on_callback(
        kv.global,
        name,
        Rc::new(move |args| {
            if let (Some(key), Some(value), Some(host)) =
                (string_arg(args, 0), convert(args), weak.upgrade())
            {
                kv2.set(&host, &key, value);
            }
            Value::Void
        }),
    );
}

fn key(args: &[Value]) -> String {
    string_arg(args, 0).unwrap_or_default()
}

fn install_key_values(host: &Rc<ShellHost>, kv: &Rc<KeyValues>) {
    getter(host, kv, "get-string", |store, args| {
        let fallback = string_arg(args, 1).unwrap_or_default();
        string(store.get(&key(args)).and_then(Json::as_str).map_or(fallback, str::to_owned))
    });
    getter(host, kv, "get-number", |store, args| {
        let fallback = number_arg(args, 1).unwrap_or(0.0);
        Value::Number(store.get(&key(args)).and_then(Json::as_f64).unwrap_or(fallback))
    });
    getter(host, kv, "get-bool", |store, args| {
        let fallback = bool_arg(args, 1).unwrap_or(false);
        Value::Bool(store.get(&key(args)).and_then(Json::as_bool).unwrap_or(fallback))
    });
    setter(host, kv, "set-string", |a| string_arg(a, 1).map(Json::String));
    setter(host, kv, "set-number", |a| number_arg(a, 1).map(|n| json!(n)));
    setter(host, kv, "set-bool", |a| bool_arg(a, 1).map(Json::Bool));

    let weak = Rc::downgrade(host);
    let kv2 = kv.clone();
    host.on_callback(
        kv.global,
        "remove",
        Rc::new(move |args| {
            if let Some(host) = weak.upgrade() {
                kv2.store.borrow_mut().remove(&key(args));
                kv2.bump(&host);
            }
            Value::Void
        }),
    );
    let weak = Rc::downgrade(host);
    let kv2 = kv.clone();
    host.on_callback(
        kv.global,
        "toggle",
        Rc::new(move |args| {
            if let Some(host) = weak.upgrade() {
                let key = key(args);
                let current = kv2.store.borrow().get(&key).and_then(Json::as_bool).unwrap_or(false);
                kv2.set(&host, &key, Json::Bool(!current));
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
    log::debug!("store: {}", path.display());
    let persistent = KeyValues::new("Store");
    *persistent.store.borrow_mut() = Store::load(&path);
    install_key_values(host, &persistent);
    install_key_values(host, &KeyValues::new("Shared"));
}
