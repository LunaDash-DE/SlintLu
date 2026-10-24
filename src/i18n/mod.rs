//! Translations: gettext catalogs compiled from `.po` files at runtime, so shells only ship
//! plain-text translations (as produced by `slint-tr-extractor`).

mod catalog;
mod locale;
mod mo;
mod po;

pub use catalog::{CompiledCatalogs, compile_catalogs};
pub use locale::{current_language, normalize_language};
pub use po::{Message, parse_po};
