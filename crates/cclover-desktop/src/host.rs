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
    crate::windows::run(app)?;
    Ok(())
}
