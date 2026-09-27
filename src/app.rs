use std::time::Instant;

use iced::futures::SinkExt;
use iced::{Element, Subscription, Task};

use crate::core::model::MonitorState;
use crate::core::{SAMPLE_INTERVAL, Sampler};
use crate::platform::{Backend, DesktopCommand};
use crate::presentation::Dashboard;
use crate::ui;

#[cfg_attr(target_os = "linux", iced_layershell::to_layer_message)]
#[derive(Debug, Clone)]
pub enum Message {
    Monitor(MonitorState),
    Desktop(DesktopCommand),
}

pub struct App {
    state: MonitorState,
    layout: ui::PanelLayout,
    surface_height: u32,
}

pub fn boot() -> App {
    let state = MonitorState::default();
    let layout = ui::PanelLayout::new(Dashboard::new(&state));
    App {
        state,
        layout,
        surface_height: ui::INITIAL_PANEL_HEIGHT,
    }
}

#[cfg(target_os = "linux")]
pub fn boot_x11() -> (App, Task<Message>) {
    (boot(), crate::platform::desktop::configure_x11_task())
}

pub fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::Monitor(state) => {
            let layout = ui::PanelLayout::new(Dashboard::new(&state));
            let next_height = layout.height();
            app.state = state;
            app.layout = layout;
            if next_height != app.surface_height {
                app.surface_height = next_height;
                #[cfg(target_os = "linux")]
                {
                    if crate::platform::desktop::is_x11() {
                        return crate::platform::desktop::resize_x11_task(
                            ui::PANEL_WIDTH,
                            next_height,
                        );
                    }
                    return Task::done(Message::SizeChange((ui::PANEL_WIDTH, next_height)));
                }
            }
            Task::none()
        }
        Message::Desktop(DesktopCommand::Quit) => iced::exit(),
        #[cfg(target_os = "linux")]
        _ => Task::none(),
    }
}

pub fn view(app: &App) -> Element<'_, Message> {
    ui::view(Dashboard::new(&app.state), &app.layout)
}

pub fn subscription(_app: &App) -> Subscription<Message> {
    let monitor = Subscription::run(monitor_stream);

    #[cfg(target_os = "linux")]
    {
        Subscription::batch([
            monitor,
            crate::platform::desktop::subscription().map(Message::Desktop),
        ])
    }

    #[cfg(not(target_os = "linux"))]
    monitor
}

fn monitor_stream() -> impl iced::futures::Stream<Item = Message> {
    iced::stream::channel(1, async move |mut output| {
        let mut monitor = Sampler::new(Backend::new());
        loop {
            let started = Instant::now();
            if output
                .send(Message::Monitor(monitor.sample()))
                .await
                .is_err()
            {
                break;
            }
            let remaining = SAMPLE_INTERVAL.saturating_sub(started.elapsed());
            if !remaining.is_zero() {
                smol::Timer::after(remaining).await;
            }
        }
    })
}
