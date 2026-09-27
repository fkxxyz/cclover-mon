use iced::futures::SinkExt;
use iced::{Element, Subscription, Task};

use crate::core::model::MonitorState;
use crate::core::{SampleCycle, Sampler};
use crate::platform::{Backend, DesktopCommand};
use crate::presentation::Dashboard;
use crate::ui;
use crate::web::StateHub;

#[cfg_attr(target_os = "linux", iced_layershell::to_layer_message)]
#[derive(Debug, Clone)]
pub enum Message {
    Monitor(MonitorState),
    Desktop(DesktopCommand),
}

pub struct App {
    state: MonitorState,
    panel: ui::PanelState,
    web_state: Option<StateHub>,
}

pub fn boot(web_state: Option<StateHub>) -> App {
    let state = MonitorState::default();
    let panel = ui::PanelState::new(Dashboard::new(&state));
    App {
        state,
        panel,
        web_state,
    }
}

#[cfg(target_os = "linux")]
pub fn boot_x11(web_state: Option<StateHub>) -> (App, Task<Message>) {
    (
        boot(web_state),
        crate::platform::desktop::configure_x11_task(),
    )
}

pub fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::Monitor(state) => {
            if let Some(web_state) = &app.web_state {
                web_state.publish(&state);
            }
            app.state = state;
            if let Some(next_height) = app.panel.update(Dashboard::new(&app.state)) {
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
    app.panel.view(Dashboard::new(&app.state))
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
            let cycle = SampleCycle::begin();
            if output
                .send(Message::Monitor(monitor.sample()))
                .await
                .is_err()
            {
                break;
            }
            let remaining = cycle.remaining();
            if !remaining.is_zero() {
                smol::Timer::after(remaining).await;
            }
        }
    })
}
