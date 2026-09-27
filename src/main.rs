use cclover_mon::{app, cli, platform, ui, web};

#[cfg(target_os = "linux")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(options) = cli::parse() else {
        return Ok(());
    };
    let http = options.http.map(web::HttpServer::start).transpose()?;
    let web_state = http.as_ref().map(web::HttpServer::state_hub);

    match platform::desktop::display_server()? {
        platform::desktop::DisplayServer::Wayland => run_wayland(web_state)?,
        platform::desktop::DisplayServer::X11 => run_x11(web_state)?,
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn run_wayland(web_state: Option<web::StateHub>) -> Result<(), iced_layershell::Error> {
    use iced::Color;
    use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
    use iced_layershell::settings::{LayerShellSettings, Settings};

    iced_layershell::disable_clipboard();
    iced_layershell::application(
        move || app::boot(web_state.clone()),
        "cclover-mon",
        app::update,
        app::view,
    )
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
            size: Some((ui::PANEL_WIDTH, ui::INITIAL_PANEL_HEIGHT)),
            margin: (16, 16, 0, 0),
            keyboard_interactivity: KeyboardInteractivity::None,
            ..LayerShellSettings::default()
        },
        ..Settings::default()
    })
    .run()
}

#[cfg(target_os = "linux")]
fn run_x11(web_state: Option<web::StateHub>) -> iced::Result {
    use iced::Color;

    iced::application(
        move || app::boot_x11(web_state.clone()),
        app::update,
        app::view,
    )
    .subscription(app::subscription)
    .theme(app_theme)
    .style(|_, _| iced::theme::Style {
        background_color: Color::TRANSPARENT,
        text_color: Color::WHITE,
    })
    .window(platform::desktop::x11_window_settings(
        ui::PANEL_WIDTH,
        ui::INITIAL_PANEL_HEIGHT,
    ))
    .run()
}

#[cfg(target_os = "windows")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(options) = cli::parse() else {
        return Ok(());
    };
    let http = options.http.map(web::HttpServer::start).transpose()?;
    let web_state = http.as_ref().map(web::HttpServer::state_hub);

    iced::application(move || app::boot(web_state.clone()), app::update, app::view)
        .subscription(app::subscription)
        .theme(app_theme)
        .window_size((ui::PANEL_WIDTH as f32, ui::INITIAL_PANEL_HEIGHT as f32))
        .transparent(true)
        .run()?;
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn app_theme(_: &app::App) -> iced::Theme {
    ui::theme()
}

#[cfg(test)]
mod architecture_tests {
    #[test]
    fn execution_adapters_do_not_own_sampling_interval_arithmetic() {
        for (path, source) in [
            ("src/app.rs", include_str!("app.rs")),
            ("src/cli.rs", include_str!("cli.rs")),
        ] {
            assert!(
                !source.contains("SAMPLE_INTERVAL"),
                "{path} must consume core sampling policy through SampleCycle instead of depending on SAMPLE_INTERVAL directly"
            );
        }
    }
}
