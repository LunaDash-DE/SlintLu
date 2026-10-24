//! The `Icons` global and the image cache shared by every global that shows icons.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, OnceLock};

use slint::Image;
use slint_interpreter::Value;
use slintlu::icons::{IconLookup, default_theme_name};

use crate::scripting::ShellHost;
use crate::scripting::bridge::{number_arg, string, string_arg};

/// Pixel size used when a global needs an icon and the shell did not ask for one.
pub const DEFAULT_SIZE: u32 = 48;

/// The process-wide icon theme lookup (also used from scanner threads).
pub fn lookup() -> Arc<IconLookup> {
    static LOOKUP: OnceLock<Arc<IconLookup>> = OnceLock::new();
    LOOKUP.get_or_init(|| Arc::new(IconLookup::new(None))).clone()
}

thread_local! {
    static IMAGES: RefCell<HashMap<PathBuf, Image>> = RefCell::default();
}

/// Whether Slint can decode the file, judging by its extension.
fn decodable(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        matches!(e.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg" | "svg" | "svgz")
    })
}

/// Loads (once) the image at `path`; an empty image when it cannot be decoded.
pub fn image_at(path: &Path) -> Image {
    if !decodable(path) {
        return Image::default();
    }
    IMAGES.with(|cache| {
        cache
            .borrow_mut()
            .entry(path.to_path_buf())
            .or_insert_with(|| {
                Image::load_from_path(path).unwrap_or_else(|_| {
                    log::debug!("cannot decode icon {}", path.display());
                    Image::default()
                })
            })
            .clone()
    })
}

/// An icon by theme name, path or `file://` URL.
pub fn icon(name: &str, size: u32) -> Image {
    lookup().lookup(name, size).map(|p| image_at(&p)).unwrap_or_default()
}

/// Drops cached images while Slint is still alive.
pub fn shutdown() {
    IMAGES.with(|cache| cache.borrow_mut().clear());
}

pub fn install(host: &Rc<ShellHost>) {
    host.set_global("Icons", "theme", string(default_theme_name()));
    host.on_callback(
        "Icons",
        "lookup",
        Rc::new(|args| {
            let name = string_arg(args, 0).unwrap_or_default();
            let size = number_arg(args, 1).filter(|s| *s > 0.0).map_or(DEFAULT_SIZE, |s| s as u32);
            Value::Image(icon(&name, size))
        }),
    );
}
