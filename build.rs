//! Embeds every file under res/ (shared shell assets) into the binary; the @slintlu library is
//! extracted together with them (see src/scripting/library.rs).

use std::fmt::Write;
use std::path::{Path, PathBuf};

fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, files);
        } else {
            files.push(path);
        }
    }
}

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let res = root.join("res");
    println!("cargo:rerun-if-changed=res");
    let mut files = Vec::new();
    collect(&res, &mut files);
    files.sort();
    let mut code =
        String::from("/// Files under res/, as (path relative to the library, contents).\n");
    code.push_str("pub const RESOURCES: &[(&str, &[u8])] = &[\n");
    for file in &files {
        println!("cargo:rerun-if-changed={}", file.display());
        let relative = file.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
        writeln!(code, "    ({relative:?}, include_bytes!({:?})),", file.display().to_string())
            .unwrap();
    }
    code.push_str("];\n");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("resources.rs");
    std::fs::write(out, code).unwrap();
}
