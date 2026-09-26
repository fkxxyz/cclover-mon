mod app;
mod core;
mod platform;
mod ui;

#[cfg(target_os = "linux")]
fn main() -> Result<(), iced_layershell::Error> {
    use iced::Color;
    use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
    use iced_layershell::settings::{LayerShellSettings, Settings};

    iced_layershell::disable_clipboard();
    iced_layershell::application(app::boot, "cclover-mon", app::update, app::view)
        .subscription(app::subscription)
        .theme(ui::theme())
        .style(|_, _| iced::theme::Style {
            background_color: Color::TRANSPARENT,
            text_color: Color::WHITE,
        })
        .settings(Settings {
            layer_settings: LayerShellSettings {
                anchor: Anchor::Top | Anchor::Right,
                layer: Layer::Bottom,
                exclusive_zone: 0,
                size: Some((390, 480)),
                margin: (16, 0, 0, 16),
                keyboard_interactivity: KeyboardInteractivity::None,
                ..LayerShellSettings::default()
            },
            ..Settings::default()
        })
        .run()
}

#[cfg(target_os = "windows")]
fn main() -> iced::Result {
    iced::application(app::boot, app::update, app::view)
        .subscription(app::subscription)
        .theme(app_theme)
        .window_size((390.0, 480.0))
        .transparent(true)
        .run()
}

#[cfg(target_os = "windows")]
fn app_theme(_: &app::App) -> iced::Theme {
    ui::theme()
}
