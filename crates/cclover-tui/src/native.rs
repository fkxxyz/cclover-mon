#![allow(unsafe_code)]

use std::ffi::c_void;
use std::io;
use std::ptr;
use std::time::Duration;

use crate::layout::RenderedFrame;

macro_rules! abi_rust_type {
    (usize) => { usize };
    (const_u8_ptr) => { *const u8 };
    (const_native_text_ptr) => { *const NativeText };
    (native_text) => { NativeText };
}

macro_rules! define_native_abi {
    (
        structs {
            $(
                $rust_struct:ident => $c_struct:ident {
                    $( $field:ident : $field_type:ident, )*
                }
            )*
        }
    ) => {
        $(
            #[repr(C)]
            struct $rust_struct {
                $( $field: abi_rust_type!($field_type), )*
            }
        )*
    };
}

include!("native_abi_spec.rs");

fn native_text(text: &str) -> NativeText {
    NativeText {
        ptr: text.as_ptr(),
        len: text.len(),
    }
}

unsafe extern "C" {
    fn cclover_tui_enter(out: *mut *mut c_void) -> i32;
    fn cclover_tui_leave(ui: *mut c_void);
    fn cclover_tui_size(ui: *mut c_void, width: *mut u32, height: *mut u32) -> i32;
    fn cclover_tui_draw(ui: *mut c_void, frame: *const NativeFrame) -> i32;
    fn cclover_tui_size_changed(ui: *mut c_void, changed: *mut i32) -> i32;
    fn cclover_tui_wait_for_quit(ui: *mut c_void, timeout_ms: u32, quit: *mut i32) -> i32;
}

pub(crate) struct Terminal {
    handle: *mut c_void,
}

impl Terminal {
    pub(crate) fn enter() -> io::Result<Self> {
        let mut handle = ptr::null_mut();
        // SAFETY: native enter initializes `handle` on success and owns no Rust memory.
        let result = unsafe { cclover_tui_enter(&mut handle) };
        check(result)?;
        if handle.is_null() {
            return Err(io::Error::other("native terminal returned a null handle"));
        }
        Ok(Self { handle })
    }

    pub(crate) fn size(&mut self) -> io::Result<(u32, u32)> {
        let mut width = 0;
        let mut height = 0;
        // SAFETY: `self.handle` is valid until Drop and both output pointers are writable.
        check(unsafe { cclover_tui_size(self.handle, &mut width, &mut height) })?;
        Ok((width, height))
    }

    pub(crate) fn draw(&mut self, frame: &RenderedFrame) -> io::Result<()> {
        let lines: Vec<_> = frame.lines.iter().map(|line| native_text(line)).collect();
        let native = NativeFrame {
            lines: if lines.is_empty() {
                ptr::null()
            } else {
                lines.as_ptr()
            },
            line_count: lines.len(),
        };
        // SAFETY: pointers in `native` reference `lines`/frame storage alive for this synchronous call.
        check(unsafe { cclover_tui_draw(self.handle, &native) })
    }

    pub(crate) fn size_changed(&mut self) -> io::Result<bool> {
        let mut changed = 0;
        // SAFETY: `self.handle` is valid until Drop and `changed` is writable for the call.
        check(unsafe { cclover_tui_size_changed(self.handle, &mut changed) })?;
        Ok(changed != 0)
    }

    pub(crate) fn wait_for_quit(&mut self, wait: Duration) -> io::Result<bool> {
        let timeout_ms = wait.as_millis().min(u128::from(u32::MAX)) as u32;
        let mut quit = 0;
        // SAFETY: `self.handle` is valid until Drop and `quit` is writable for the call.
        check(unsafe { cclover_tui_wait_for_quit(self.handle, timeout_ms, &mut quit) })?;
        Ok(quit != 0)
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        // SAFETY: the handle was returned by native enter and is released exactly once here.
        unsafe { cclover_tui_leave(self.handle) };
    }
}

fn check(code: i32) -> io::Result<()> {
    if code == 0 {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(code))
    }
}
