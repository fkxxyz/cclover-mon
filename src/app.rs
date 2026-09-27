use iced::futures::SinkExt;
use iced::{Element, Subscription, Task};

use crate::core::model::MonitorState;
use crate::core::{SampleCycle, Sampler};
use crate::platform::{Backend, DesktopCommand};
use crate::presentation::Dashboard;
use crate::ui;
use crate::web::StateHub;

#[derive(Debug, Clone)]
pub enum Message {
    Monitor(MonitorState),
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

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[derive(Clone)]
pub struct DesktopApp {
    web_state: Option<StateHub>,
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
impl DesktopApp {
    pub fn new(web_state: Option<StateHub>) -> Self {
        Self { web_state }
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
impl crate::platform::desktop::DesktopApplication for DesktopApp {
    type State = App;
    type Message = Message;

    fn boot(&self) -> (Self::State, Task<Self::Message>) {
        (boot(self.web_state.clone()), Task::none())
    }

    fn update(&self, state: &mut Self::State, message: Self::Message) -> Task<Self::Message> {
        update(state, message)
    }

    fn view<'a>(&self, state: &'a Self::State) -> Element<'a, Self::Message> {
        view(state)
    }

    fn subscription(&self, state: &Self::State) -> Subscription<Self::Message> {
        subscription(state)
    }

    fn theme(&self, _state: &Self::State) -> iced::Theme {
        ui::theme()
    }

    fn initial_surface_size(&self) -> (u32, u32) {
        (ui::PANEL_WIDTH, ui::INITIAL_PANEL_HEIGHT)
    }

    fn surface_size(&self, state: &Self::State) -> (u32, u32) {
        (ui::PANEL_WIDTH, state.panel.surface_height())
    }

    fn handle_desktop_command(
        &self,
        _state: &mut Self::State,
        command: DesktopCommand,
    ) -> Task<Self::Message> {
        match command {
            DesktopCommand::Quit => iced::exit(),
        }
    }
}

pub fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::Monitor(state) => {
            if let Some(web_state) = &app.web_state {
                web_state.publish(&state);
            }
            app.state = state;
            app.panel.update(Dashboard::new(&app.state));
            Task::none()
        }
    }
}

pub fn view(app: &App) -> Element<'_, Message> {
    app.panel.view(Dashboard::new(&app.state))
}

pub fn subscription(_app: &App) -> Subscription<Message> {
    Subscription::run(monitor_stream)
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
