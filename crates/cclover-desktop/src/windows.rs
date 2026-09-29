#![allow(unsafe_code)]

use std::error::Error;
use std::ffi::c_void;

use crate::native::{DesktopApp, HostCallbacks, NativeContext};

unsafe extern "C" {
    fn cclover_win32_run(context: *mut c_void, callbacks: *const HostCallbacks) -> i32;
}

pub fn run(app: DesktopApp) -> Result<(), Box<dyn Error>> {
    let mut context = Box::new(NativeContext::new(app.receiver, None));
    let callbacks = HostCallbacks::new();
    // SAFETY: context and callbacks remain alive for the full synchronous native message loop.
    let result = unsafe { cclover_win32_run(context.as_ptr(), &callbacks) };
    if result == 0 {
        Ok(())
    } else {
        Err(format!("Win32 desktop host failed with error {result}").into())
    }
}
