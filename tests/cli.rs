//! End-to-end tests of the `slintlu` executable that need no compositor.

use std::path::Path;
use std::process::Command;

fn slintlu() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_slintlu"));
    // Keep the extracted library out of the developer's home directory.
    cmd.env("XDG_DATA_HOME", std::env::temp_dir().join("slintlu-test-data"));
    cmd
}

#[test]
fn starter_config_compiles() {
    let out = slintlu().args(["check", "examples/starter"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains("ok (Bar)"));
}

#[test]
fn errors_fail_the_check() {
    let dir = tempfile::tempdir().unwrap();
    let shell = dir.path().join("shell.slint");
    std::fs::write(
        &shell,
        "import { PanelWindow } from \"@slintlu\";\nexport component X inherits PanelWindow { nope: 1; }\n",
    )
    .unwrap();
    let out = slintlu().arg("check").arg(&shell).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("has errors"));
}

#[test]
fn init_writes_a_working_config() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("cfg");
    let out = slintlu().arg("init").arg("--dir").arg(&target).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(Path::new(&target.join("shell.slint")).is_file());

    // A second init refuses to overwrite.
    let again = slintlu().arg("init").arg("--dir").arg(&target).output().unwrap();
    assert!(!again.status.success());

    let check = slintlu().arg("check").arg(&target).output().unwrap();
    assert!(check.status.success(), "{}", String::from_utf8_lossy(&check.stderr));
}

#[test]
fn library_path_contains_the_library() {
    let out = slintlu().arg("library-path").output().unwrap();
    assert!(out.status.success());
    let dir = String::from_utf8(out.stdout).unwrap();
    assert!(Path::new(dir.trim()).join("slintlu.slint").is_file());
}
