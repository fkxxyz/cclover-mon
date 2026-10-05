#![cfg(target_os = "linux")]

use std::path::PathBuf;
use std::process::Command;

#[test]
fn cairo_text_measurement_reuse_matches_fallback_and_expires_after_execute() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output = std::env::temp_dir().join(format!(
        "cclover-native-cairo-text-measurement-test-{}",
        std::process::id()
    ));
    let pkg_config = Command::new("pkg-config")
        .args(["--cflags", "--libs", "cairo"])
        .output()
        .expect("query Cairo compiler flags");
    assert!(pkg_config.status.success(), "pkg-config must resolve Cairo");
    let flags = String::from_utf8(pkg_config.stdout).expect("Cairo flags must be UTF-8");
    let out_dir = PathBuf::from(env!("OUT_DIR"));
    let mut command = Command::new("cc");
    command
        .arg("-std=gnu11")
        .arg("-Wall")
        .arg("-Wextra")
        .arg("-Werror")
        .arg("-I")
        .arg(&out_dir)
        .arg("-I")
        .arg(manifest.join("native/linux"))
        .arg(manifest.join("native/linux/render.c"))
        .arg(manifest.join("tests/native/cairo_text_measurement_test.c"));
    for flag in flags.split_whitespace() {
        command.arg(flag);
    }
    command.arg("-lm");
    let status = command
        .arg("-o")
        .arg(&output)
        .status()
        .expect("compile Cairo text measurement test");
    assert!(status.success(), "Cairo text measurement test must compile");

    let status = Command::new(&output)
        .status()
        .expect("run Cairo text measurement test");
    let _ = std::fs::remove_file(&output);
    assert!(
        status.success(),
        "Cairo text measurement reuse rules must hold"
    );
}
