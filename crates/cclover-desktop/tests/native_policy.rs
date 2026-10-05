#![cfg(target_os = "linux")]

use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn native_policies_are_deterministic_without_desktop_runtimes() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    run_c_test(
        &manifest,
        "wayland-lifecycle",
        &["native/linux/wayland_lifecycle.c"],
        "tests/native/wayland_lifecycle_test.c",
        "native/linux",
    );
    run_c_test(
        &manifest,
        "x11-lifecycle",
        &["native/linux/x11_lifecycle.c"],
        "tests/native/x11_lifecycle_test.c",
        "native/linux",
    );
    run_c_test(
        &manifest,
        "windows-message-policy",
        &["native/windows/message_policy.c"],
        "tests/native/windows_message_policy_test.c",
        "native/windows",
    );
    run_c_test(
        &manifest,
        "wayland-buffer-policy",
        &[],
        "tests/native/wayland_buffer_policy_test.c",
        "native/linux",
    );
    run_c_test(
        &manifest,
        "dbusmenu-policy",
        &["native/linux/dbusmenu_policy.c"],
        "tests/native/dbusmenu_policy_test.c",
        "native/linux",
    );
}

fn run_c_test(manifest: &Path, name: &str, sources: &[&str], test: &str, include: &str) {
    let output =
        std::env::temp_dir().join(format!("cclover-native-{name}-test-{}", std::process::id()));
    let mut command = Command::new("cc");
    command
        .arg("-std=c11")
        .arg("-Wall")
        .arg("-Wextra")
        .arg("-Werror")
        .arg("-I")
        .arg(manifest.join(include));
    for source in sources {
        command.arg(manifest.join(source));
    }
    let status = command
        .arg(manifest.join(test))
        .arg("-o")
        .arg(&output)
        .status()
        .unwrap_or_else(|error| panic!("compile {name} policy test: {error}"));
    assert!(status.success(), "{name} policy test must compile");

    let status = Command::new(&output)
        .status()
        .unwrap_or_else(|error| panic!("run {name} policy test: {error}"));
    let _ = std::fs::remove_file(&output);
    assert!(status.success(), "{name} policy rules must hold");
}
