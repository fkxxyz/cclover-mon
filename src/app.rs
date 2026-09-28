use iced::futures::stream::BoxStream;
use iced::futures::{SinkExt, StreamExt};
use iced::{Element, Subscription, Task};
use std::sync::{Arc, Mutex};

use crate::core::model::MonitorState;
use crate::platform::DesktopCommand;
use crate::presentation::Dashboard;
use crate::runtime::StateSource;
use crate::ui;

#[derive(Debug, Clone)]
pub enum Message {
    Monitor(MonitorState),
}

pub struct App {
    state: MonitorState,
    panel: ui::PanelState,
}

pub fn boot() -> App {
    let state = MonitorState::default();
    let panel = ui::PanelState::new(Dashboard::new(&state));
    App { state, panel }
}

#[derive(Clone)]
pub struct DesktopApp {
    states: StateSource,
}

impl DesktopApp {
    pub fn new(states: StateSource) -> Self {
        Self { states }
    }
}

impl crate::platform::desktop::DesktopApplication for DesktopApp {
    type State = App;
    type Message = Message;

    fn boot(&self) -> (Self::State, Task<Self::Message>) {
        (boot(), Task::none())
    }

    fn update(&self, state: &mut Self::State, message: Self::Message) -> Task<Self::Message> {
        update(state, message)
    }

    fn view<'a>(&self, state: &'a Self::State) -> Element<'a, Self::Message> {
        view(state)
    }

    fn subscription(&self, _state: &Self::State) -> Subscription<Self::Message> {
        monitor_subscription(self.states.clone())
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
            app.state = state;
            app.panel.update(Dashboard::new(&app.state));
            Task::none()
        }
    }
}

pub fn view(app: &App) -> Element<'_, Message> {
    app.panel.view(Dashboard::new(&app.state))
}

fn monitor_subscription(states: StateSource) -> Subscription<Message> {
    Subscription::run_with(states, monitor_stream)
}

fn monitor_stream(states: &StateSource) -> BoxStream<'static, Message> {
    let receiver = Arc::new(Mutex::new(states.subscribe()));
    iced::stream::channel(1, async move |mut output| {
        loop {
            let receiver = Arc::clone(&receiver);
            let next = smol::unblock(move || {
                receiver
                    .lock()
                    .expect("desktop monitor receiver lock poisoned")
                    .recv()
            })
            .await;
            let Ok(state) = next else {
                break;
            };
            if output.send(Message::Monitor(state)).await.is_err() {
                break;
            }
        }
    })
    .boxed()
}
