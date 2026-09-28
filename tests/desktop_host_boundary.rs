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
