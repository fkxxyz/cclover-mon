use iced::{Point, Size, Task, window};
use x11rb::connection::Connection;
use x11rb::properties::WmHints;
use x11rb::protocol::shape::{ConnectionExt as _, SK, SO};
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ClipOrdering, ConnectionExt as _, EventMask, PropMode,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

const PANEL_MARGIN: f32 = 16.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayServer {
    Wayland,
    X11,
}

pub fn display_server() -> Result<DisplayServer, String> {
    if non_empty_env("WAYLAND_DISPLAY") {
        return Ok(DisplayServer::Wayland);
    }
    if non_empty_env("DISPLAY") {
        return Ok(DisplayServer::X11);
    }
    Err("no supported display server found: WAYLAND_DISPLAY and DISPLAY are both unset".to_owned())
}

pub fn is_x11() -> bool {
    matches!(display_server(), Ok(DisplayServer::X11))
}

pub fn x11_window_settings(width: u32, height: u32) -> window::Settings {
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

pub fn configure_x11_task<Message: Send + 'static>() -> Task<Message> {
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

pub fn resize_x11_task<Message: Send + 'static>(width: u32, height: u32) -> Task<Message> {
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
    let root = conn.setup().roots[screen_num].root;

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

    let net_wm_state = intern_atom(&conn, b"_NET_WM_STATE")?;
    let states = [
        intern_atom(&conn, b"_NET_WM_STATE_SKIP_TASKBAR")?,
        intern_atom(&conn, b"_NET_WM_STATE_SKIP_PAGER")?,
        intern_atom(&conn, b"_NET_WM_STATE_BELOW")?,
    ];

    conn.change_property32(
        PropMode::REPLACE,
        window,
        net_wm_state,
        AtomEnum::ATOM,
        &states,
    )
    .map_err(|error| error.to_string())?;

    let mut hints = WmHints::new();
    hints.input = Some(false);
    hints
        .set(&conn, window)
        .map_err(|error| error.to_string())?;

    let net_wm_desktop = intern_atom(&conn, b"_NET_WM_DESKTOP")?;
    conn.change_property32(
        PropMode::REPLACE,
        window,
        net_wm_desktop,
        AtomEnum::CARDINAL,
        &[u32::MAX],
    )
    .map_err(|error| error.to_string())?;

    let geometry = conn
        .get_geometry(window)
        .map_err(|error| error.to_string())?
        .reply()
        .map_err(|error| error.to_string())?;
    let screen = &conn.setup().roots[screen_num];
    let margin = PANEL_MARGIN as i32;
    let x = (i32::from(screen.width_in_pixels) - i32::from(geometry.width) - margin).max(0);

    conn.configure_window(
        window,
        &x11rb::protocol::xproto::ConfigureWindowAux::new()
            .x(x)
            .y(margin)
            .stack_mode(x11rb::protocol::xproto::StackMode::BELOW),
    )
    .map_err(|error| error.to_string())?;

    request_net_wm_state(&conn, root, window, net_wm_state, states[0], states[1])?;
    request_net_wm_state(&conn, root, window, net_wm_state, states[2], 0)?;

    let net_moveresize_window = intern_atom(&conn, b"_NET_MOVERESIZE_WINDOW")?;
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

    conn.flush().map_err(|error| error.to_string())?;
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
    use super::top_right_position;
    use iced::{Point, Size};

    #[test]
    fn top_right_position_preserves_margin() {
        assert_eq!(
            top_right_position(Size::new(320.0, 600.0), Size::new(1920.0, 1080.0)),
            Point::new(1584.0, 16.0)
        );
    }
}
