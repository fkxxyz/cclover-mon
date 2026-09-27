#[test]
fn native_desktop_hosting_stays_in_platform_desktop() {
    for (path, source) in [
        ("src/main.rs", include_str!("../src/main.rs")),
        ("src/app.rs", include_str!("../src/app.rs")),
    ] {
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
                "{path} must not own native desktop-host policy ({forbidden}); keep it in platform desktop integration"
            );
        }
    }
}
