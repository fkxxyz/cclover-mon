use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[test]
fn linux_tray_uses_status_notifier_name_watch_contract() {
    let source = include_str!("../crates/cclover-desktop/native/linux_tray.c");

    assert!(source.contains("g_bus_watch_name_on_connection"));
    assert!(source.contains("org.kde.StatusNotifierWatcher"));
    assert!(source.contains("RegisterStatusNotifierItem"));
}

#[test]
fn linux_dbusmenu_v4_declares_grouped_methods() {
    let source = include_str!("../crates/cclover-desktop/native/linux_tray.c");

    for method in ["EventGroup", "AboutToShowGroup"] {
        assert!(
            source.contains(&format!("<method name='{method}'>")),
            "D-BusMenu version 4 declaration must expose {method}"
        );
    }
}

#[test]
fn native_desktop_hosting_stays_in_desktop_crate() {
    for (path, source) in [("src/main.rs", include_str!("../src/main.rs"))] {
        for forbidden in [
            "iced_layershell",
            "DisplayServer",
            "WAYLAND_DISPLAY",
            "configure_x11",
            "resize_x11",
            "x11_window_settings",
            "iced::application",
            ".window_size(",
        ] {
            assert!(
                !source.contains(forbidden),
                "{path} must not own native desktop-host policy ({forbidden}); keep it in the desktop crate"
            );
        }
    }
}

#[test]
fn graphical_renderers_do_not_regain_cross_platform_gui_frameworks() {
    for (name, manifest) in [
        ("workspace root", include_str!("../Cargo.toml")),
        (
            "native desktop",
            include_str!("../crates/cclover-desktop/Cargo.toml"),
        ),
        ("Web", include_str!("../crates/cclover-web-ui/Cargo.toml")),
    ] {
        for forbidden in ["iced", "winit", "wgpu"] {
            assert!(
                !manifest.contains(forbidden),
                "{name} manifest must not pull cross-platform GUI framework dependency {forbidden} back in"
            );
        }
    }

    let lock = include_str!("../Cargo.lock");
    for forbidden_package_prefix in ["iced", "winit", "wgpu"] {
        assert!(
            !lock.contains(&format!("name = \"{forbidden_package_prefix}")),
            "resolved dependency graph must not contain {forbidden_package_prefix} packages"
        );
    }

    let desktop_manifest = include_str!("../crates/cclover-desktop/Cargo.toml");
    assert!(
        !desktop_manifest.contains("cclover-web-ui"),
        "native desktop must not depend on the browser renderer"
    );

    for (name, source) in [
        (
            "native",
            include_str!("../crates/cclover-desktop/src/native.rs"),
        ),
        (
            "linux",
            include_str!("../crates/cclover-desktop/src/linux.rs"),
        ),
        (
            "windows",
            include_str!("../crates/cclover-desktop/src/windows.rs"),
        ),
    ] {
        for forbidden in ["iced::", "winit::", "wgpu::", "cclover_web_ui"] {
            assert!(
                !source.contains(forbidden),
                "{name} desktop source must consume shared Scene/native host only ({forbidden})"
            );
        }
    }
}

#[test]
fn native_scene_abi_has_one_declarative_authority() {
    let rust_bridge = rust_native();
    let build_script = include_str!("../crates/cclover-desktop/build.rs");
    let spec = include_str!("../crates/cclover-desktop/src/native_abi_spec.rs");
    let handwritten_header = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("crates/cclover-desktop/native/native_scene.h");

    assert!(rust_bridge.contains("include!(\"../native_abi_spec.rs\")"));
    assert!(build_script.contains("include!(\"src/native_abi_spec.rs\")"));
    assert!(spec.contains("NativeCommand => CcloverCommand"));
    assert!(
        !handwritten_header.exists(),
        "native scene ABI must not regain a separately maintained C header"
    );
}

#[test]
fn graphical_layout_is_shared_and_renderer_independent() {
    let layout = include_str!("../crates/cclover-ui/src/layout.rs");
    let tree = include_str!("../crates/cclover-ui/src/tree.rs");
    let web_source = include_str!("../crates/cclover-web-ui/src/lib.rs");
    let web = web_source
        .split_once("#[cfg(test)]")
        .expect("Web renderer must keep tests after production code")
        .0;

    assert!(layout.contains("CellWidth::Fixed"));
    assert!(layout.contains("CellWidth::Fill"));
    assert!(!layout.contains("TextMeasurer"));
    assert!(!layout.contains("text.width"));
    assert!(tree.contains("pub enum CellWidth"));
    assert!(web.contains("pub fn render(scene: &Scene)"));
    assert!(!web.contains("display:grid"));
    assert!(!web.contains("display:flex"));
}

#[test]
fn linux_desktop_surface_preserves_panel_window_policy() {
    let source = linux_native();

    for required in [
        "_NET_WM_STATE_SKIP_TASKBAR",
        "_NET_WM_STATE_SKIP_PAGER",
        "_NET_WM_STATE_BELOW",
        "XShapeCombineRectangles",
        "ShapeInput",
        "ZWLR_LAYER_SHELL_V1_LAYER_BOTTOM",
        "ZWLR_LAYER_SURFACE_V1_ANCHOR_TOP",
        "ZWLR_LAYER_SURFACE_V1_ANCHOR_RIGHT",
        "zwlr_layer_surface_v1_set_exclusive_zone",
        "wl_surface_set_input_region",
    ] {
        assert!(
            source.contains(required),
            "Linux desktop host must preserve native panel policy primitive: {required}"
        );
    }
}

#[test]
fn windows_desktop_surface_preserves_panel_window_policy() {
    let source = windows_native();

    for required in [
        "GetShellWindow()",
        "GW_OWNER",
        "WS_EX_TOOLWINDOW",
        "WS_EX_NOACTIVATE",
        "WS_EX_LAYERED",
        "WS_EX_TRANSPARENT",
        "SetLayeredWindowAttributes",
        "LWA_ALPHA",
        "WM_NCHITTEST",
        "HTTRANSPARENT",
        "HWND_BOTTOM",
        "RegisterWindowMessageW",
        "TaskbarCreated",
    ] {
        assert!(
            source.contains(required),
            "Windows desktop host must preserve native panel policy primitive: {required}"
        );
    }
}
fn rust_native() -> &'static str {
    static SOURCE: OnceLock<String> = OnceLock::new();
    SOURCE
        .get_or_init(|| read_rust_module_tree("crates/cclover-desktop/src/native.rs"))
        .as_str()
}

fn linux_native() -> &'static str {
    static SOURCE: OnceLock<String> = OnceLock::new();
    SOURCE
        .get_or_init(|| read_c_host_tree("crates/cclover-desktop/native/linux_host.c"))
        .as_str()
}

fn windows_native() -> &'static str {
    static SOURCE: OnceLock<String> = OnceLock::new();
    SOURCE
        .get_or_init(|| read_c_host_tree("crates/cclover-desktop/native/windows_host.c"))
        .as_str()
}

fn read_rust_module_tree(root_file: &str) -> String {
    let root = workspace_path(root_file);
    let module_dir = root.with_extension("");
    let mut files = vec![root];
    collect_source_files(&module_dir, &["rs"], &mut files);
    read_sources(files)
}

fn read_c_host_tree(root_file: &str) -> String {
    let root = workspace_path(root_file);
    let stem = root
        .file_stem()
        .and_then(|stem| stem.to_str())
        .and_then(|stem| stem.strip_suffix("_host"))
        .expect("native C host root must follow <platform>_host.c naming");
    let module_dir = root
        .parent()
        .expect("native C host root must have a parent directory")
        .join(stem);
    let mut files = vec![root];
    collect_source_files(&module_dir, &["c", "h", "inc"], &mut files);
    read_sources(files)
}

fn workspace_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn collect_source_files(root: &Path, extensions: &[&str], files: &mut Vec<PathBuf>) {
    if !root.exists() {
        return;
    }
    let mut entries = fs::read_dir(root)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", root.display()))
        .map(|entry| {
            entry
                .expect("source directory entry must be readable")
                .path()
        })
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_source_files(&path, extensions, files);
            continue;
        }
        if path.file_name().and_then(|name| name.to_str()) == Some("tests.rs") {
            continue;
        }
        let extension = path.extension().and_then(|extension| extension.to_str());
        if extension.is_some_and(|extension| extensions.contains(&extension)) {
            files.push(path);
        }
    }
}

fn read_sources(mut files: Vec<PathBuf>) -> String {
    files.sort();
    files
        .into_iter()
        .map(|path| {
            fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
        })
        .collect::<Vec<_>>()
        .join("\n")
}
