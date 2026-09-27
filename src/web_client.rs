use iced::{Element, Size, Subscription, Task};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{EventSource, MessageEvent};

use crate::core::model::MonitorState;
use crate::presentation::Dashboard;
use crate::ui;

#[derive(Debug, Clone)]
enum Message {
    Monitor(MonitorState),
}

struct WebApp {
    state: MonitorState,
    panel: ui::PanelState,
}

pub fn run() -> iced::Result {
    iced::application(boot, update, view)
        .subscription(subscription)
        .theme(theme)
        .style(|_, _| iced::theme::Style {
            background_color: iced::Color::TRANSPARENT,
            text_color: iced::Color::WHITE,
        })
        .window_size((ui::PANEL_WIDTH as f32, ui::INITIAL_PANEL_HEIGHT as f32))
        .resizable(false)
        .transparent(true)
        .run()
}

fn theme(_: &WebApp) -> iced::Theme {
    ui::theme()
}

fn boot() -> WebApp {
    let state = MonitorState::default();
    let panel = ui::PanelState::new(Dashboard::new(&state));
    WebApp { state, panel }
}

fn update(app: &mut WebApp, message: Message) -> Task<Message> {
    match message {
        Message::Monitor(state) => {
            app.state = state;
            let Some(next_height) = app.panel.update(Dashboard::new(&app.state)) else {
                return Task::none();
            };

            iced::window::latest().and_then(move |id| {
                iced::window::resize(id, Size::new(ui::PANEL_WIDTH as f32, next_height as f32))
            })
        }
    }
}

fn view(app: &WebApp) -> Element<'_, Message> {
    app.panel.view(Dashboard::new(&app.state))
}

fn subscription(_app: &WebApp) -> Subscription<Message> {
    Subscription::run(monitor_stream)
}

fn monitor_stream() -> impl iced::futures::Stream<Item = Message> {
    iced::stream::channel(4, async move |mut output| {
        let source = match EventSource::new("/events") {
            Ok(source) => source,
            Err(_) => return,
        };

        let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
            let Some(payload) = event.data().as_string() else {
                return;
            };
            let Ok(state) = serde_json::from_str::<MonitorState>(&payload) else {
                return;
            };
            let _ = output.try_send(Message::Monitor(state));
        });
        source.set_onmessage(Some(on_message.as_ref().unchecked_ref()));

        iced::futures::future::pending::<()>().await;
        source.close();
        drop(on_message);
    })
}
