#![cfg(target_os = "linux")]

use std::path::PathBuf;
use std::process::Command;

#[test]
fn windows_geometry_rules_are_deterministic() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output = std::env::temp_dir().join(format!(
        "cclover-windows-geometry-test-{}",
        std::process::id()
    ));

    let status = Command::new("cc")
        .arg("-std=c11")
        .arg("-Wall")
        .arg("-Wextra")
        .arg("-Werror")
        .arg("-I")
        .arg(manifest.join("native"))
        .arg(manifest.join("native/windows_geometry.c"))
        .arg(manifest.join("tests/native/windows_geometry_test.c"))
        .arg("-lm")
        .arg("-o")
        .arg(&output)
        .status()
        .expect("compile Windows geometry rule test");
    assert!(status.success(), "Windows geometry rule test must compile");

    let status = Command::new(&output)
        .status()
        .expect("run Windows geometry rule test");
    let _ = std::fs::remove_file(&output);
    assert!(status.success(), "Windows geometry rules must hold");
}
