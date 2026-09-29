#![allow(unsafe_code)]

use iced::window::raw_window_handle::RawWindowHandle;
use iced::{Element, Point, Size, Subscription, Task, window};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetWindowLongPtrW, HWND_BOTTOM, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SetWindowLongPtrW, SetWindowPos, WS_EX_APPWINDOW, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW,
};

use crate::host::DesktopApplication;

const PANEL_MARGIN: f32 = 16.0;

struct HostedState<A: DesktopApplication> {
    adapter: A,
    app: A::State,
    surface_size: (u32, u32),
}

#[derive(Debug, Clone)]
enum HostMessage<Message> {
    App(Message),
}

pub(crate) fn run<A: DesktopApplication>(app: A) -> iced::Result {
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
    .window(window::Settings {
        size: Size::new(width as f32, height as f32),
        position: window::Position::SpecificWith(top_right_position),
        resizable: false,
        minimizable: false,
        closeable: false,
        decorations: false,
        transparent: true,
        level: window::Level::AlwaysOnBottom,
        platform_specific: window::settings::PlatformSpecific {
            skip_taskbar: true,
            ..window::settings::PlatformSpecific::default()
        },
        ..window::Settings::default()
    })
    .run()
}

fn host_boot<A: DesktopApplication>(adapter: A) -> (HostedState<A>, Task<HostMessage<A::Message>>) {
    let (state, app_task) = adapter.boot();
    let surface_size = adapter.surface_size(&state);
    let shell_task = window::latest()
        .and_then(|id| Task::batch([window::enable_mouse_passthrough(id), apply_shell_policy(id)]));
    (
        HostedState {
            adapter,
            app: state,
            surface_size,
        },
        Task::batch([app_task.map(HostMessage::App), shell_task]),
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
        let desired = Size::new(next_size.0 as f32, next_size.1 as f32);
        window::resize::<()>(id, desired)
            .then(move |_| window::monitor_size(id))
            .then(move |monitor| {
                monitor.map_or_else(
                    || apply_shell_policy(id),
                    |monitor| {
                        window::move_to::<()>(id, top_right_position(desired, monitor))
                            .then(move |_| apply_shell_policy(id))
                    },
                )
            })
    });
    Task::batch([app_task, resize_task])
}

fn apply_shell_policy<Message: Send + 'static>(id: window::Id) -> Task<Message> {
    window::run(id, |window| {
        let Ok(handle) = window.window_handle() else {
            return;
        };
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return;
        };
        let hwnd = handle.hwnd.get() as HWND;
        configure_monitor_window(hwnd);
    })
    .discard()
}

fn configure_monitor_window(hwnd: HWND) {
    // SAFETY: hwnd comes from the live Iced/winit Win32 window on the UI thread.
    unsafe {
        let before = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        #[cfg(target_pointer_width = "32")]
        let requested = (before & !(WS_EX_APPWINDOW as i32))
            | WS_EX_TOOLWINDOW as i32
            | WS_EX_NOACTIVATE as i32;
        #[cfg(target_pointer_width = "64")]
        let requested = (before & !(WS_EX_APPWINDOW as isize))
            | WS_EX_TOOLWINDOW as isize
            | WS_EX_NOACTIVATE as isize;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, requested);
        SetWindowPos(
            hwnd,
            HWND_BOTTOM,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
}

fn top_right_position(window: Size, monitor: Size) -> Point {
    Point::new(
        (monitor.width - window.width - PANEL_MARGIN).max(0.0),
        PANEL_MARGIN,
    )
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
