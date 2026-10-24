//! The `Apps` global: installed applications, search and launching.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use slint_interpreter::Value;
use slintlu::launcher::{self, AppEntry, LaunchHistory};

use super::icons::{self, DEFAULT_SIZE};
use crate::config::Watcher;
use crate::scripting::bridge::{list_value, number_arg, string, string_arg, struct_value};
use crate::scripting::{ShellHost, host};

/// A scanned application with its icon file resolved off the UI thread.
struct ScannedApp {
    entry: AppEntry,
    icon: Option<PathBuf>,
}

struct AppsState {
    apps: Vec<AppEntry>,
    values: Vec<Value>,
    history: LaunchHistory,
}

thread_local! {
    static STATE: RefCell<Option<Rc<RefCell<AppsState>>>> = const { RefCell::new(None) };
    static WATCHER: RefCell<Option<Watcher>> = const { RefCell::new(None) };
}

fn state() -> Option<Rc<RefCell<AppsState>>> {
    STATE.with(|s| s.borrow().clone())
}

fn value(app: &ScannedApp) -> Value {
    let e = &app.entry;
    struct_value([
        ("id", string(&e.id)),
        ("name", string(&e.name)),
        ("generic-name", string(&e.generic_name)),
        ("comment", string(&e.comment)),
        ("icon", Value::Image(app.icon.as_deref().map(icons::image_at).unwrap_or_default())),
        ("icon-name", string(&e.icon)),
        ("terminal", Value::Bool(e.terminal)),
        ("categories", list_value(e.categories.iter().map(string))),
        (
            "actions",
            list_value(
                e.actions
                    .iter()
                    .map(|a| struct_value([("id", string(&a.id)), ("name", string(&a.name))])),
            ),
        ),
    ])
}

/// Scans in the background and publishes the result on the event loop.
fn rescan() {
    std::thread::spawn(|| {
        let language = slintlu::i18n::current_language();
        let lookup = icons::lookup();
        let apps: Vec<ScannedApp> =
            launcher::scan(&launcher::application_dirs(), language.as_deref())
                .into_iter()
                .map(|entry| ScannedApp { icon: lookup.lookup(&entry.icon, DEFAULT_SIZE), entry })
                .collect();
        host::post(move |host| {
            let Some(state) = state() else { return };
            let values: Vec<Value> = apps.iter().map(value).collect();
            host.set_global("Apps", "all", list_value(values.iter().cloned()));
            let mut state = state.borrow_mut();
            state.apps = apps.into_iter().map(|a| a.entry).collect();
            state.values = values;
            log::debug!("{} applications", state.apps.len());
        });
    });
}

/// The icon of the application with this desktop file id (or `StartupWMClass` / app id).
pub fn icon_for(app_id: &str) -> Option<slint::Image> {
    let state = state()?;
    let state = state.borrow();
    let app_id = app_id.trim_end_matches(".desktop");
    let index = state.apps.iter().position(|a| {
        a.id.trim_end_matches(".desktop").eq_ignore_ascii_case(app_id)
            || a.startup_wm_class.eq_ignore_ascii_case(app_id)
    })?;
    match &state.values[index] {
        Value::Struct(s) => match s.get_field("icon") {
            Some(Value::Image(image)) => Some(image.clone()),
            _ => None,
        },
        _ => None,
    }
}

fn launch(id: &str, action: Option<&str>) {
    let Some(state) = state() else { return };
    let mut state = state.borrow_mut();
    let Some(app) = state.apps.iter().find(|a| a.id == id).cloned() else {
        return log::warn!("no application with id {id}");
    };
    match launcher::launch(&app, action) {
        Ok(()) => state.history.record(id),
        Err(e) => log::error!("{}: {e}", app.name),
    }
}

/// Releases cached values while Slint is still alive.
pub fn shutdown() {
    STATE.with(|s| s.borrow_mut().take());
    WATCHER.with(|w| w.borrow_mut().take());
}

pub fn install(host: &Rc<ShellHost>) {
    let initial = AppsState {
        apps: Vec::new(),
        values: Vec::new(),
        history: LaunchHistory::load(LaunchHistory::default_path()),
    };
    STATE.with(|s| *s.borrow_mut() = Some(Rc::new(RefCell::new(initial))));
    rescan();

    let dirs: Vec<PathBuf> =
        launcher::application_dirs().into_iter().filter(|d| d.is_dir()).collect();
    match Watcher::new(&dirs, rescan) {
        Ok(watcher) => WATCHER.with(|w| *w.borrow_mut() = Some(watcher)),
        Err(e) => log::warn!("applications will not refresh automatically: {e}"),
    }

    host.on_callback(
        "Apps",
        "search",
        Rc::new(|args| {
            let Some(state) = state() else { return list_value([]) };
            let state = state.borrow();
            let query = string_arg(args, 0).unwrap_or_default();
            let limit = number_arg(args, 1).filter(|l| *l > 0.0).map_or(usize::MAX, |l| l as usize);
            let results = launcher::search(&state.apps, &query, &state.history, limit);
            list_value(results.into_iter().filter_map(|app| {
                let index = state.apps.iter().position(|a| a.id == app.id)?;
                state.values.get(index).cloned()
            }))
        }),
    );
    host.on_callback(
        "Apps",
        "launch",
        Rc::new(|args| {
            if let Some(id) = string_arg(args, 0) {
                launch(&id, None);
            }
            Value::Void
        }),
    );
    host.on_callback(
        "Apps",
        "launch-action",
        Rc::new(|args| {
            if let (Some(id), Some(action)) = (string_arg(args, 0), string_arg(args, 1)) {
                launch(&id, Some(&action));
            }
            Value::Void
        }),
    );
}
