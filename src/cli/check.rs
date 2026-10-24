//! `slintlu check`: compile a configuration and print diagnostics.

use std::path::PathBuf;

use crate::config;
use crate::scripting;

pub fn check(path: Option<PathBuf>) -> Result<(), String> {
    let config = config::resolve_config(path.as_deref())?;
    match scripting::compile(&config) {
        Ok(compiled) => {
            let names: Vec<_> = compiled.result.component_names().collect();
            println!("{}: ok ({})", config.display(), names.join(", "));
            Ok(())
        }
        Err(diagnostics) => {
            slint_interpreter::print_diagnostics(&diagnostics);
            Err(format!("{} has errors", config.display()))
        }
    }
}
