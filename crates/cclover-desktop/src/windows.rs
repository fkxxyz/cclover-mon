#![allow(unsafe_code)]

use std::error::Error;
use std::ffi::c_void;

use crate::native::{DesktopApp, HostCallbacks, NativeContext, NativeStateBridge};

unsafe extern "C" {
    fn cclover_win32_run(context: *mut c_void, callbacks: *const HostCallbacks) -> i32;
    fn cclover_win32_prepare_wake() -> u32;
    fn cclover_win32_wake(thread_id: u32);
}

pub fn run(app: DesktopApp) -> Result<(), Box<dyn Error>> {
    // SAFETY: called on the UI thread before the bridge can post wake messages.
    let thread_id = unsafe { cclover_win32_prepare_wake() };
    let bridge = NativeStateBridge::spawn(app.receiver, move || {
        // SAFETY: the thread id belongs to this synchronous desktop host loop.
        unsafe { cclover_win32_wake(thread_id) };
    });
    let mut context = Box::new(NativeContext::new(bridge.pending(), None));
    let callbacks = HostCallbacks::new();
    // SAFETY: context and callbacks remain alive for the full synchronous native message loop.
    let result = unsafe { cclover_win32_run(context.as_ptr(), &callbacks) };
    if result == 0 {
        Ok(())
    } else {
        Err(format!("Win32 desktop host failed with error {result}").into())
    }
}
