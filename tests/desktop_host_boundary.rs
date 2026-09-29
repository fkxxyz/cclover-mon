#[test]
fn linux_tray_tolerates_late_status_notifier_watcher() {
    let source = include_str!("../crates/cclover-desktop/src/linux.rs");

    assert!(
        source.contains("assume_sni_available(true)"),
        "Linux tray startup must tolerate StatusNotifierWatcher appearing after the application starts"
    );
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
                "{name} desktop source must consume NativeScene/native host only ({forbidden})"
            );
        }
    }
}

#[test]
fn native_scene_abi_has_one_declarative_authority() {
    let rust_bridge = include_str!("../crates/cclover-desktop/src/native.rs");
    let build_script = include_str!("../crates/cclover-desktop/build.rs");
    let spec = include_str!("../crates/cclover-desktop/src/native_abi_spec.rs");
    let handwritten_header = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("crates/cclover-desktop/native/native_scene.h");

    assert!(rust_bridge.contains("include!(\"native_abi_spec.rs\")"));
    assert!(build_script.contains("include!(\"src/native_abi_spec.rs\")"));
    assert!(spec.contains("NativeCommand => CcloverCommand"));
    assert!(
        !handwritten_header.exists(),
        "native scene ABI must not regain a separately maintained C header"
    );
}

#[test]
fn native_text_layout_uses_realized_renderer_metrics() {
    let scene = include_str!("../crates/cclover-ui/src/scene.rs");
    let linux = include_str!("../crates/cclover-desktop/native/linux_host.c");
    let windows = include_str!("../crates/cclover-desktop/native/windows_host.c");

    assert!(scene.contains("NativeTextMeasurer"));
    assert!(scene.contains("text.width(&cell.text, cell.size, cell.weight)"));
    assert!(
        !scene.contains("estimated_text_width"),
        "native scene lowering must not regain guessed font metrics"
    );

    for (name, source, realization, measurement) in [
        (
            "Linux",
            linux,
            "cclover_select_font",
            "cairo_text_extents(host->measure_cr",
        ),
        (
            "Windows",
            windows,
            "cclover_font(host",
            "GetTextExtentPoint32W(host->measure_dc",
        ),
    ] {
        assert!(
            source.contains(realization) && source.contains(measurement),
            "{name} native host must measure through its realized drawing font"
        );
        assert!(
            source.contains(
                "host->callbacks->scene(host->context, host, cclover_measure_text, scene)"
            ),
            "{name} native host must supply measurement to shared scene lowering"
        );
    }
}

#[test]
fn linux_desktop_surface_preserves_panel_window_policy() {
    let source = include_str!("../crates/cclover-desktop/native/linux_host.c");

    for required in [
        "_NET_WM_STATE_SKIP_TASKBAR",
        "_NET_WM_STATE_SKIP_PAGER",
        "_NET_WM_STATE_BELOW",
        "XShapeCombineRectangles(display, window, ShapeInput",
        "ZWLR_LAYER_SHELL_V1_LAYER_BOTTOM",
        "ZWLR_LAYER_SURFACE_V1_ANCHOR_TOP | ZWLR_LAYER_SURFACE_V1_ANCHOR_RIGHT",
        "zwlr_layer_surface_v1_set_exclusive_zone(host.layer_surface, 0)",
        "wl_surface_set_input_region(host.surface, empty_input)",
    ] {
        assert!(
            source.contains(required),
            "Linux desktop host must preserve native panel policy primitive: {required}"
        );
    }
}

#[test]
fn windows_desktop_surface_preserves_panel_window_policy() {
    let source = include_str!("../crates/cclover-desktop/native/windows_host.c");

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
        "RegisterWindowMessageW(L\"TaskbarCreated\")",
        "message == host->taskbar_created",
    ] {
        assert!(
            source.contains(required),
            "Windows desktop host must preserve native panel policy primitive: {required}"
        );
    }
}
