//! Compiling a shell configuration with the Slint interpreter.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

use slint_interpreter::{CompilationResult, Compiler, Diagnostic};

use super::library;

/// Globals the runtime must be able to reach. The interpreter only exposes globals exported by
/// the root file, so the user's file is compiled through a small generated root that re-exports
/// both the user's components and these globals.
const RUNTIME_GLOBALS: &[&str] = &[
    "Shell",
    "SlintLuRuntime",
    "Clock",
    "System",
    "Battery",
    "Audio",
    "Workspaces",
    "Notifications",
    "Media",
    "Network",
    "Power",
    "Theme",
    "I18n",
];

/// A successful compilation and the diagnostics (warnings) that came with it.
pub struct Compiled {
    pub result: CompilationResult,
}

/// Compiles `config`; on failure returns the diagnostics to show.
pub fn compile(config: &Path) -> Result<Compiled, Vec<Diagnostic>> {
    let library = library::install().map_err(|e| {
        log::error!("cannot install the @slintlu library: {e}");
        Vec::new()
    })?;
    let root = write_root(config, &library).map_err(|e| {
        log::error!("cannot write the generated root file: {e}");
        Vec::new()
    })?;

    let mut compiler = Compiler::new();
    compiler.set_translation_domain(super::globals::TRANSLATION_DOMAIN.to_owned());
    compiler.set_library_paths(HashMap::from([(library::NAME.to_owned(), library)]));
    if let Some(dir) = config.parent() {
        compiler.set_include_paths(vec![dir.to_path_buf()]);
    }
    let result = pollster::block_on(compiler.build_from_path(&root));

    // The generated root re-exports globals the user may already export; that is fine.
    let diagnostics: Vec<Diagnostic> =
        result.diagnostics().filter(|d| d.source_file() != Some(root.as_path())).collect();
    if result.has_errors() {
        Err(diagnostics)
    } else {
        if !diagnostics.is_empty() {
            slint_interpreter::print_diagnostics(&diagnostics);
        }
        Ok(Compiled { result })
    }
}

fn write_root(config: &Path, library: &Path) -> std::io::Result<PathBuf> {
    let mut hasher = DefaultHasher::new();
    config.hash(&mut hasher);
    let dir = library.parent().unwrap_or(Path::new("."));
    let root = dir.join(format!("root-{:016x}.slint", hasher.finish()));
    let source = format!(
        "export * from \"{}\";\nexport {{ {} }} from \"@{}\";\n",
        escape(config),
        RUNTIME_GLOBALS.join(", "),
        library::NAME,
    );
    if std::fs::read_to_string(&root).ok() != Some(source.clone()) {
        std::fs::write(&root, source)?;
    }
    Ok(root)
}

fn escape(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_escaped_for_slint_strings() {
        assert_eq!(escape(Path::new("/a/b \"c\".slint")), "/a/b \\\"c\\\".slint");
    }
}
