use iced::{Element, Subscription, Task, Theme};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopCommand {
    Quit,
}

pub trait DesktopApplication: Clone + 'static {
    type State: 'static;
    type Message: Clone + std::fmt::Debug + Send + 'static;

    fn boot(&self) -> (Self::State, Task<Self::Message>);
    fn update(&self, state: &mut Self::State, message: Self::Message) -> Task<Self::Message>;
    fn view<'a>(&self, state: &'a Self::State) -> Element<'a, Self::Message>;
    fn subscription(&self, state: &Self::State) -> Subscription<Self::Message>;
    fn theme(&self, state: &Self::State) -> Theme;
    fn initial_surface_size(&self) -> (u32, u32);
    fn surface_size(&self, state: &Self::State) -> (u32, u32);
    fn handle_desktop_command(
        &self,
        state: &mut Self::State,
        command: DesktopCommand,
    ) -> Task<Self::Message>;
}

#[cfg(target_os = "linux")]
pub fn run<A: DesktopApplication>(app: A) -> Result<(), Box<dyn std::error::Error>> {
    crate::linux::run(app)
}

#[cfg(target_os = "windows")]
pub fn run<A: DesktopApplication>(app: A) -> Result<(), Box<dyn std::error::Error>> {
    windows::run(app)?;
    Ok(())
}

#[cfg(target_os = "windows")]
mod windows {
    use iced::{Element, Size, Subscription, Task, window};

    use super::DesktopApplication;

    struct HostedState<A: DesktopApplication> {
        adapter: A,
        app: A::State,
        surface_size: (u32, u32),
    }

    #[derive(Debug, Clone)]
    enum HostMessage<Message> {
        App(Message),
    }

    pub(super) fn run<A: DesktopApplication>(app: A) -> iced::Result {
        let (width, height) = app.initial_surface_size();
        iced::application(
            {
                let app = app.clone();
                move || host_boot(app.clone())
            },
            host_update::<A>,
            host_view::<A>,
        )
        .subscription(host_subscription::<A>)
        .theme(host_theme::<A>)
        .window_size((width as f32, height as f32))
        .transparent(true)
        .run()
    }

    fn host_boot<A: DesktopApplication>(
        adapter: A,
    ) -> (HostedState<A>, Task<HostMessage<A::Message>>) {
        let (state, app_task) = adapter.boot();
        let surface_size = adapter.surface_size(&state);
        (
            HostedState {
                adapter,
                app: state,
                surface_size,
            },
            app_task.map(HostMessage::App),
        )
    }

    fn host_update<A: DesktopApplication>(
        state: &mut HostedState<A>,
        message: HostMessage<A::Message>,
    ) -> Task<HostMessage<A::Message>> {
        let HostMessage::App(message) = message;
        let app_task = state
            .adapter
            .update(&mut state.app, message)
            .map(HostMessage::App);

        let next_size = state.adapter.surface_size(&state.app);
        if next_size == state.surface_size {
            return app_task;
        }
        state.surface_size = next_size;

        let resize_task = window::latest().and_then(move |id| {
            window::resize(id, Size::new(next_size.0 as f32, next_size.1 as f32))
        });
        Task::batch([app_task, resize_task])
    }

    fn host_view<A: DesktopApplication>(
        state: &HostedState<A>,
    ) -> Element<'_, HostMessage<A::Message>> {
        state.adapter.view(&state.app).map(HostMessage::App)
    }

    fn host_subscription<A: DesktopApplication>(
        state: &HostedState<A>,
    ) -> Subscription<HostMessage<A::Message>> {
        state.adapter.subscription(&state.app).map(HostMessage::App)
    }

    fn host_theme<A: DesktopApplication>(state: &HostedState<A>) -> iced::Theme {
        state.adapter.theme(&state.app)
    }
}
