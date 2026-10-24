//! The `Files` global: directory listings, images and background thumbnails.

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;

use slint_interpreter::Value;
use slintlu::files;
use slintlu::util::xdg;

use super::icons;
use crate::scripting::bridge::{list_value, number_arg, string, string_arg, struct_value};
use crate::scripting::{ShellHost, host};

thread_local! {
    static PENDING: RefCell<HashSet<PathBuf>> = RefCell::default();
    static REVISION: RefCell<u32> = const { RefCell::new(0) };
}

fn thumbnail(path: PathBuf, size: u32) -> Value {
    let cached = files::thumbnail_path(&path, size);
    if cached.is_file() {
        return Value::Image(icons::image_at(&cached));
    }
    // Generate once in the background, then bump the revision so bindings ask again.
    if PENDING.with(|p| p.borrow_mut().insert(path.clone())) {
        std::thread::spawn(move || {
            if let Err(e) = files::thumbnail(&path, size) {
                log::debug!("thumbnail: {e}");
            }
            host::post(move |host| {
                PENDING.with(|p| p.borrow_mut().remove(&path));
                let revision = REVISION.with(|r| {
                    *r.borrow_mut() += 1;
                    *r.borrow()
                });
                host.set_global("Files", "revision", Value::Number(revision.into()));
            });
        });
    }
    Value::Image(slint::Image::default())
}

pub fn install(host: &Rc<ShellHost>) {
    host.set_global("Files", "home", string(xdg::home_dir().to_string_lossy()));
    host.on_callback(
        "Files",
        "list",
        Rc::new(|args| {
            let dir = files::expand_home(&string_arg(args, 0).unwrap_or_default());
            let extensions = string_arg(args, 1).unwrap_or_default().to_ascii_lowercase();
            let extensions: Vec<&str> =
                extensions.split(',').map(str::trim).filter(|e| !e.is_empty()).collect();
            list_value(files::list(&dir, &extensions).into_iter().map(|e| {
                struct_value([
                    ("path", string(e.path.to_string_lossy())),
                    ("name", string(&e.name)),
                    ("is-dir", Value::Bool(e.is_dir)),
                ])
            }))
        }),
    );
    host.on_callback(
        "Files",
        "exists",
        Rc::new(|args| {
            Value::Bool(files::expand_home(&string_arg(args, 0).unwrap_or_default()).exists())
        }),
    );
    host.on_callback(
        "Files",
        "image",
        Rc::new(|args| {
            let path = files::expand_home(&string_arg(args, 0).unwrap_or_default());
            Value::Image(icons::image_at(&path))
        }),
    );
    host.on_callback(
        "Files",
        "thumbnail",
        Rc::new(|args| {
            let path = files::expand_home(&string_arg(args, 0).unwrap_or_default());
            let size = number_arg(args, 1).filter(|s| *s > 0.0).map_or(256, |s| s as u32);
            if !path.is_file() {
                return Value::Image(slint::Image::default());
            }
            thumbnail(path, size)
        }),
    );
}
