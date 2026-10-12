use std::path::PathBuf;

fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        return;
    }

    println!("cargo:rerun-if-changed=assets/windows/cclover-mon.rc");
    println!("cargo:rerun-if-changed=assets/windows/cclover-mon.ico");
    let icon_path =
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("package manifest directory"))
            .join("assets/windows/cclover-mon.ico")
            .canonicalize()
            .expect("Windows application icon must exist")
            .display()
            .to_string()
            .replace('\\', "/");
    embed_resource::compile_for(
        "assets/windows/cclover-mon.rc",
        ["cclover-mon"],
        [format!("CCLOVER_ICON_PATH=\"{icon_path}\"")],
    )
    .manifest_required()
    .expect("failed to embed Windows application icon");
}
