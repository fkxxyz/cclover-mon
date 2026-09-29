#![allow(unsafe_code)]

use std::error::Error;
use std::ffi::c_void;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ksni::blocking::TrayMethods as _;

use crate::native::{DesktopApp, HostCallbacks, NativeContext};

#[derive(Clone)]
struct TrayIcon {
    quit: Arc<AtomicBool>,
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
                    tray.quit.store(true, Ordering::Relaxed);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DisplayServer {
    Wayland,
    X11,
}

unsafe extern "C" {
    fn cclover_linux_wayland_run(context: *mut c_void, callbacks: *const HostCallbacks) -> i32;
    fn cclover_linux_x11_run(context: *mut c_void, callbacks: *const HostCallbacks) -> i32;
}

pub fn run(app: DesktopApp) -> Result<(), Box<dyn Error>> {
    let display = display_server()?;
    let quit = Arc::new(AtomicBool::new(false));
    let tray = TrayIcon {
        quit: Arc::clone(&quit),
    };
    let _tray_handle = match tray.assume_sni_available(true).spawn() {
        Ok(handle) => Some(handle),
        Err(error) => {
            eprintln!("cclover-mon: system tray unavailable: {error}");
            None
        }
    };

    let mut context = Box::new(NativeContext::new(app.receiver, Some(quit)));
    let callbacks = HostCallbacks::new();
    // SAFETY: context and callbacks remain alive for the full synchronous native host loop.
    let result = unsafe {
        match display {
            DisplayServer::Wayland => cclover_linux_wayland_run(context.as_ptr(), &callbacks),
            DisplayServer::X11 => cclover_linux_x11_run(context.as_ptr(), &callbacks),
        }
    };
    if result == 0 {
        Ok(())
    } else {
        Err(format!("Linux native desktop host failed with error {result}").into())
    }
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

fn non_empty_env(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| !value.is_empty())
}
