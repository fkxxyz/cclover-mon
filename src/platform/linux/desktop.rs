use iced::futures::SinkExt;
use iced::{Color, Element, Event, Point, Size, Subscription, Task, Theme, event, window};
use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
use iced_layershell::settings::{LayerShellSettings, Settings};
use ksni::blocking::TrayMethods as _;
use x11rb::connection::Connection;
use x11rb::properties::WmHints;
use x11rb::protocol::shape::{ConnectionExt as _, SK, SO};
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ClipOrdering, ConnectionExt as _, EventMask, PropMode,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

use crate::platform::{DesktopCommand, desktop::DesktopApplication};

const PANEL_MARGIN: f32 = 16.0;

struct HostedState<A: DesktopApplication> {
    adapter: A,
    app: A::State,
    surface_geometry: SurfaceGeometry,
    display_server: DisplayServer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SurfaceGeometry {
    desired: (u32, u32),
    realized: Option<(u32, u32)>,
}

impl SurfaceGeometry {
    fn new(desired: (u32, u32)) -> Self {
        Self {
            desired,
            realized: None,
        }
    }

    fn update_desired(&mut self, desired: (u32, u32)) -> Option<(u32, u32)> {
        if desired == self.desired {
            return None;
        }

        self.desired = desired;
        self.resize_target()
    }

    fn observe_realized(&mut self, realized: (u32, u32)) -> Option<(u32, u32)> {
        self.realized = Some(realized);
        self.resize_target()
    }

    fn resize_target(self) -> Option<(u32, u32)> {
        (self.realized != Some(self.desired)).then_some(self.desired)
    }
}

#[iced_layershell::to_layer_message]
#[derive(Debug, Clone)]
enum HostMessage<Message> {
    App(Message),
    Desktop(DesktopCommand),
    Surface(Event),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DisplayServer {
    Wayland,
    X11,
}

pub fn run<A: DesktopApplication>(app: A) -> Result<(), Box<dyn std::error::Error>> {
    match display_server()? {
        DisplayServer::Wayland => run_wayland(app)?,
        DisplayServer::X11 => run_x11(app)?,
    }
    Ok(())
}

fn run_wayland<A: DesktopApplication>(app: A) -> Result<(), iced_layershell::Error> {
    let initial_size = app.initial_surface_size();
    iced_layershell::disable_clipboard();

    iced_layershell::application(
        {
            let app = app.clone();
            move || host_boot(app.clone(), DisplayServer::Wayland)
        },
        "cclover-mon",
        host_update::<A>,
        host_view::<A>,
    )
    .subscription(host_subscription::<A>)
    .theme(host_theme::<A>)
    .style(|_, _| iced::theme::Style {
        background_color: Color::TRANSPARENT,
        text_color: Color::WHITE,
    })
    .settings(Settings {
        layer_settings: LayerShellSettings {
            anchor: Anchor::Top | Anchor::Right,
            layer: Layer::Bottom,
            exclusive_zone: 0,
            size: Some(initial_size),
            margin: (PANEL_MARGIN as i32, PANEL_MARGIN as i32, 0, 0),
            keyboard_interactivity: KeyboardInteractivity::None,
            ..LayerShellSettings::default()
        },
        ..Settings::default()
    })
    .run()
}

fn run_x11<A: DesktopApplication>(app: A) -> iced::Result {
    let (width, height) = app.initial_surface_size();
    iced::application(
        {
            let app = app.clone();
            move || host_boot(app.clone(), DisplayServer::X11)
        },
        host_update::<A>,
        host_view::<A>,
    )
    .subscription(host_subscription::<A>)
    .theme(host_theme::<A>)
    .style(|_, _| iced::theme::Style {
        background_color: Color::TRANSPARENT,
        text_color: Color::WHITE,
    })
    .window(x11_window_settings(width, height))
    .run()
}

fn host_boot<A: DesktopApplication>(
    adapter: A,
    display_server: DisplayServer,
) -> (HostedState<A>, Task<HostMessage<A::Message>>) {
    let (state, app_task) = adapter.boot();
    let surface_geometry = SurfaceGeometry::new(adapter.surface_size(&state));
    let host_task = match display_server {
        DisplayServer::Wayland => Task::none(),
        DisplayServer::X11 => configure_x11_task(),
    };

    (
        HostedState {
            adapter,
            app: state,
            surface_geometry,
            display_server,
        },
        Task::batch([app_task.map(HostMessage::App), host_task]),
    )
}

fn host_update<A: DesktopApplication>(
    state: &mut HostedState<A>,
    message: HostMessage<A::Message>,
) -> Task<HostMessage<A::Message>> {
    let (app_task, resize_target) = match message {
        HostMessage::App(message) => {
            let task = state.adapter.update(&mut state.app, message);
            let desired = state.adapter.surface_size(&state.app);
            (task, state.surface_geometry.update_desired(desired))
        }
        HostMessage::Desktop(command) => {
            let task = state
                .adapter
                .handle_desktop_command(&mut state.app, command);
            let desired = state.adapter.surface_size(&state.app);
            (task, state.surface_geometry.update_desired(desired))
        }
        HostMessage::Surface(event) => {
            let resize_target = surface_event_size(&event)
                .and_then(|size| state.surface_geometry.observe_realized(size));
            (Task::none(), resize_target)
        }
        _ => (Task::none(), None),
    };

    let app_task = app_task.map(HostMessage::App);
    let resize_task = resize_target
        .map(|size| resize_surface_task::<A::Message>(state.display_server, size))
        .unwrap_or_else(Task::none);
    Task::batch([app_task, resize_task])
}

fn surface_event_size(event: &Event) -> Option<(u32, u32)> {
    let size = match event {
        Event::Window(window::Event::Opened { size, .. })
        | Event::Window(window::Event::Resized(size)) => *size,
        _ => return None,
    };

    Some((size.width.round() as u32, size.height.round() as u32))
}

fn resize_surface_task<Message: Clone + std::fmt::Debug + Send + 'static>(
    display_server: DisplayServer,
    size: (u32, u32),
) -> Task<HostMessage<Message>> {
    match display_server {
        DisplayServer::Wayland => Task::done(HostMessage::SizeChange(size)),
        DisplayServer::X11 => resize_x11_task(size.0, size.1),
    }
}

fn host_view<A: DesktopApplication>(
    state: &HostedState<A>,
) -> Element<'_, HostMessage<A::Message>> {
    state.adapter.view(&state.app).map(HostMessage::App)
}

fn host_subscription<A: DesktopApplication>(
    state: &HostedState<A>,
) -> Subscription<HostMessage<A::Message>> {
    Subscription::batch([
        state.adapter.subscription(&state.app).map(HostMessage::App),
        subscription().map(HostMessage::Desktop),
        event::listen().map(HostMessage::Surface),
    ])
}

fn host_theme<A: DesktopApplication>(state: &HostedState<A>) -> Theme {
    state.adapter.theme(&state.app)
}

#[derive(Clone)]
struct TrayIcon {
    commands: smol::channel::Sender<DesktopCommand>,
}

impl ksni::Tray for TrayIcon {
    fn id(&self) -> String {
        "cclover-mon".to_owned()
    }

    fn title(&self) -> String {
        "cclover-mon".to_owned()
    }

    fn icon_name(&self) -> String {
        "utilities-system-monitor".to_owned()
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::StandardItem;

        vec![
            StandardItem {
                label: "Quit".to_owned(),
                icon_name: "application-exit".to_owned(),
                activate: Box::new(|tray: &mut TrayIcon| {
                    let _ = tray.commands.try_send(DesktopCommand::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

fn subscription() -> Subscription<DesktopCommand> {
    Subscription::run(tray_stream)
}

fn tray_stream() -> impl iced::futures::Stream<Item = DesktopCommand> {
    iced::stream::channel(1, async move |mut output| {
        let (commands, receiver) = smol::channel::bounded(4);
        let tray = TrayIcon { commands };
        let _handle = match tray.spawn() {
            Ok(handle) => handle,
            Err(error) => {
                eprintln!("cclover-mon: system tray unavailable: {error}");
                std::future::pending::<()>().await;
                unreachable!();
            }
        };

        while let Ok(command) = receiver.recv().await {
            if output.send(command).await.is_err() {
                break;
            }
        }
    })
}

fn display_server() -> Result<DisplayServer, String> {
    if non_empty_env("WAYLAND_DISPLAY") {
        return Ok(DisplayServer::Wayland);
    }
    if non_empty_env("DISPLAY") {
        return Ok(DisplayServer::X11);
    }
    Err("no supported display server found: WAYLAND_DISPLAY and DISPLAY are both unset".to_owned())
}

fn x11_window_settings(width: u32, height: u32) -> window::Settings {
    window::Settings {
        size: Size::new(width as f32, height as f32),
        position: window::Position::SpecificWith(top_right_position),
        resizable: false,
        decorations: false,
        transparent: true,
        level: window::Level::AlwaysOnBottom,
        platform_specific: window::settings::PlatformSpecific {
            application_id: "cclover-mon".to_owned(),
            override_redirect: false,
        },
        ..window::Settings::default()
    }
}

fn configure_x11_task<Message: Send + 'static>() -> Task<Message> {
    window::latest()
        .and_then(|id| window::raw_id::<Message>(id))
        .then(|raw_id| {
            Task::future(async move {
                if let Err(error) = configure_x11_window(raw_id) {
                    eprintln!("cclover-mon: X11 window configuration failed: {error}");
                }
            })
        })
        .discard()
}

fn resize_x11_task<Message: Send + 'static>(width: u32, height: u32) -> Task<Message> {
    window::latest().and_then(move |id| window::resize(id, Size::new(width as f32, height as f32)))
}

fn top_right_position(window_size: Size, monitor_size: Size) -> Point {
    Point::new(
        (monitor_size.width - window_size.width - PANEL_MARGIN).max(0.0),
        PANEL_MARGIN,
    )
}

fn configure_x11_window(raw_id: u64) -> Result<(), String> {
    let window = u32::try_from(raw_id).map_err(|_| format!("invalid X11 window id {raw_id}"))?;
    let (conn, screen_num) = RustConnection::connect(None).map_err(|error| error.to_string())?;
    let screen = &conn.setup().roots[screen_num];

    configure_x11_input_policy(&conn, window)?;
    configure_x11_window_manager_policy(&conn, screen.root, window)?;
    configure_x11_workspace_policy(&conn, window)?;
    configure_x11_placement(&conn, screen.root, window, screen.width_in_pixels)?;

    conn.flush().map_err(|error| error.to_string())?;
    Ok(())
}

fn configure_x11_input_policy(conn: &RustConnection, window: u32) -> Result<(), String> {
    conn.shape_rectangles(
        SO::SET,
        SK::INPUT,
        ClipOrdering::UNSORTED,
        window,
        0,
        0,
        &[],
    )
    .map_err(|error| error.to_string())?;

    let mut hints = WmHints::new();
    hints.input = Some(false);
    hints
        .set(conn, window)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn configure_x11_window_manager_policy(
    conn: &RustConnection,
    root: u32,
    window: u32,
) -> Result<(), String> {
    let net_wm_state = intern_atom(conn, b"_NET_WM_STATE")?;
    let states = [
        intern_atom(conn, b"_NET_WM_STATE_SKIP_TASKBAR")?,
        intern_atom(conn, b"_NET_WM_STATE_SKIP_PAGER")?,
        intern_atom(conn, b"_NET_WM_STATE_BELOW")?,
    ];

    conn.change_property32(
        PropMode::REPLACE,
        window,
        net_wm_state,
        AtomEnum::ATOM,
        &states,
    )
    .map_err(|error| error.to_string())?;
    request_net_wm_state(conn, root, window, net_wm_state, states[0], states[1])?;
    request_net_wm_state(conn, root, window, net_wm_state, states[2], 0)
}

fn configure_x11_workspace_policy(conn: &RustConnection, window: u32) -> Result<(), String> {
    let net_wm_desktop = intern_atom(conn, b"_NET_WM_DESKTOP")?;
    conn.change_property32(
        PropMode::REPLACE,
        window,
        net_wm_desktop,
        AtomEnum::CARDINAL,
        &[u32::MAX],
    )
    .map(|_| ())
    .map_err(|error| error.to_string())
}

fn configure_x11_placement(
    conn: &RustConnection,
    root: u32,
    window: u32,
    screen_width: u16,
) -> Result<(), String> {
    let geometry = conn
        .get_geometry(window)
        .map_err(|error| error.to_string())?
        .reply()
        .map_err(|error| error.to_string())?;
    let margin = PANEL_MARGIN as i32;
    let x = (i32::from(screen_width) - i32::from(geometry.width) - margin).max(0);

    conn.configure_window(
        window,
        &x11rb::protocol::xproto::ConfigureWindowAux::new()
            .x(x)
            .y(margin)
            .stack_mode(x11rb::protocol::xproto::StackMode::BELOW),
    )
    .map_err(|error| error.to_string())?;

    request_x11_placement(conn, root, window, x, margin)
}

fn request_x11_placement(
    conn: &RustConnection,
    root: u32,
    window: u32,
    x: i32,
    margin: i32,
) -> Result<(), String> {
    let net_moveresize_window = intern_atom(conn, b"_NET_MOVERESIZE_WINDOW")?;
    let gravity_static = 10_u32;
    let moveresize_flags = gravity_static | (1 << 8) | (1 << 9) | (1 << 12);
    conn.send_event(
        false,
        root,
        EventMask::SUBSTRUCTURE_NOTIFY | EventMask::SUBSTRUCTURE_REDIRECT,
        x11rb::protocol::xproto::ClientMessageEvent::new(
            32,
            window,
            net_moveresize_window,
            [moveresize_flags, x as u32, margin as u32, 0, 0],
        ),
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn request_net_wm_state(
    conn: &RustConnection,
    root: u32,
    window: u32,
    net_wm_state: Atom,
    first: Atom,
    second: Atom,
) -> Result<(), String> {
    const ADD: u32 = 1;
    const SOURCE_APPLICATION: u32 = 1;

    conn.send_event(
        false,
        root,
        EventMask::SUBSTRUCTURE_NOTIFY | EventMask::SUBSTRUCTURE_REDIRECT,
        x11rb::protocol::xproto::ClientMessageEvent::new(
            32,
            window,
            net_wm_state,
            [ADD, first, second, SOURCE_APPLICATION, 0],
        ),
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn intern_atom(conn: &RustConnection, name: &[u8]) -> Result<Atom, String> {
    conn.intern_atom(false, name)
        .map_err(|error| error.to_string())?
        .reply()
        .map(|reply| reply.atom)
        .map_err(|error| error.to_string())
}

fn non_empty_env(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{SurfaceGeometry, top_right_position};
    use iced::{Point, Size};

    #[test]
    fn top_right_position_preserves_margin() {
        assert_eq!(
            top_right_position(Size::new(320.0, 600.0), Size::new(1920.0, 1080.0)),
            Point::new(1584.0, 16.0)
        );
    }

    #[test]
    fn desired_size_change_requests_resize_before_first_realized_size() {
        let mut geometry = SurfaceGeometry::new((390, 480));

        assert_eq!(geometry.update_desired((390, 672)), Some((390, 672)));
    }

    #[test]
    fn late_initial_configure_retries_current_desired_size() {
        let mut geometry = SurfaceGeometry::new((390, 480));
        assert_eq!(geometry.update_desired((390, 672)), Some((390, 672)));

        assert_eq!(geometry.observe_realized((390, 480)), Some((390, 672)));
        assert_eq!(geometry.observe_realized((390, 672)), None);
    }

    #[test]
    fn later_compositor_resize_reapplies_desired_size() {
        let mut geometry = SurfaceGeometry::new((390, 672));
        assert_eq!(geometry.observe_realized((390, 672)), None);

        assert_eq!(geometry.observe_realized((390, 480)), Some((390, 672)));
    }
}
