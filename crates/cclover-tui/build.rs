fn main() {
    println!("cargo:rerun-if-changed=native/tui.c");
    cc::Build::new()
        .file("native/tui.c")
        .warnings(true)
        .compile("cclover_tui_native");
}
