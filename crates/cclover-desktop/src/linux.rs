#![allow(unsafe_code)]

use std::error::Error;
use std::ffi::c_void;
use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::native::{DesktopApp, HostCallbacks, NativeContext};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DisplayServer {
    Wayland,
    X11,
}

unsafe extern "C" {
    fn cclover_linux_wayland_run(context: *mut c_void, callbacks: *const HostCallbacks) -> i32;
    fn cclover_linux_x11_run(context: *mut c_void, callbacks: *const HostCallbacks) -> i32;
    fn cclover_linux_tray_start(
        quit_context: *mut c_void,
        quit_fn: unsafe extern "C" fn(*mut c_void),
        out: *mut *mut c_void,
    ) -> i32;
    fn cclover_linux_tray_stop(tray: *mut c_void);
}

struct NativeTray {
    handle: *mut c_void,
    quit_context: *mut Arc<AtomicBool>,
}

impl NativeTray {
    fn start(quit: Arc<AtomicBool>) -> Option<Self> {
        let quit_context = Box::into_raw(Box::new(quit));
        let mut handle = ptr::null_mut();
        // SAFETY: `quit_context` remains owned by this wrapper until the native tray thread is stopped.
        let result = unsafe {
            cclover_linux_tray_start(
                quit_context.cast::<c_void>(),
                tray_quit_callback,
                &mut handle,
            )
        };
        if result == 0 && !handle.is_null() {
            Some(Self {
                handle,
                quit_context,
            })
        } else {
            // SAFETY: native start did not retain the callback context when it reported failure.
            unsafe { drop(Box::from_raw(quit_context)) };
            None
        }
    }
}

impl Drop for NativeTray {
    fn drop(&mut self) {
        // SAFETY: the handle was returned by `cclover_linux_tray_start` and is stopped exactly once.
        unsafe { cclover_linux_tray_stop(self.handle) };
        // SAFETY: tray stop joins its callback thread, so no native callback can access this Arc now.
        unsafe { drop(Box::from_raw(self.quit_context)) };
    }
}

unsafe extern "C" fn tray_quit_callback(context: *mut c_void) {
    // SAFETY: native tray stores the pointer supplied by NativeTray::start until tray stop completes.
    let quit = unsafe { &*context.cast::<Arc<AtomicBool>>() };
    quit.store(true, Ordering::Relaxed);
}

pub fn run(app: DesktopApp) -> Result<(), Box<dyn Error>> {
    let display = display_server()?;
    let quit = Arc::new(AtomicBool::new(false));
    let _tray = NativeTray::start(Arc::clone(&quit));

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
