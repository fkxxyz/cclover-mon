use std::time::Instant;

use iced::futures::SinkExt;
use iced::{Element, Subscription, Task};

use crate::core::model::MonitorState;
use crate::core::{SAMPLE_INTERVAL, Sampler};
use crate::platform::Backend;
use crate::ui;

#[cfg_attr(target_os = "linux", iced_layershell::to_layer_message)]
#[derive(Debug, Clone)]
pub enum Message {
    Monitor(MonitorState),
}

#[derive(Default)]
pub struct App {
    state: MonitorState,
    panel_height: u32,
}

pub fn boot() -> App {
    App {
        state: MonitorState::default(),
        panel_height: 480,
    }
}

pub fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::Monitor(state) => {
            let next_height = ui::panel_height(&state);
            app.state = state;
            if next_height != app.panel_height {
                app.panel_height = next_height;
                #[cfg(target_os = "linux")]
                return Task::done(Message::SizeChange((390, next_height)));
            }
            Task::none()
        }
        #[cfg(target_os = "linux")]
        _ => Task::none(),
    }
}

pub fn view(app: &App) -> Element<'_, Message> {
    ui::view(&app.state)
}

pub fn subscription(_app: &App) -> Subscription<Message> {
    Subscription::run(monitor_stream)
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
