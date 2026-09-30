#![allow(unsafe_code)]

use std::ffi::c_void;
use std::io;
use std::ptr;
use std::time::Duration;

#[derive(Default)]
pub(crate) struct Panel {
    pub(crate) title: String,
    pub(crate) rows: Vec<String>,
    pub(crate) history: Vec<u64>,
}

#[derive(Default)]
pub(crate) struct Frame {
    pub(crate) cpu: Panel,
    pub(crate) memory: Panel,
    pub(crate) gpu: Panel,
    pub(crate) temperatures: Panel,
    pub(crate) fans: Panel,
    pub(crate) disks: Panel,
    pub(crate) networks: Panel,
}

macro_rules! abi_rust_type {
    (usize) => { usize };
    (const_u8_ptr) => { *const u8 };
    (const_u64_ptr) => { *const u64 };
    (native_text) => { NativeText };
    (const_native_text_ptr) => { *const NativeText };
    (native_panel) => { NativePanel };
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

struct PanelView {
    rows: Vec<NativeText>,
}

impl PanelView {
    fn new(panel: &Panel) -> Self {
        Self {
            rows: panel.rows.iter().map(|row| native_text(row)).collect(),
        }
    }

    fn native(&self, panel: &Panel) -> NativePanel {
        NativePanel {
            title: native_text(&panel.title),
            rows: if self.rows.is_empty() {
                ptr::null()
            } else {
                self.rows.as_ptr()
            },
            row_count: self.rows.len(),
            history: if panel.history.is_empty() {
                ptr::null()
            } else {
                panel.history.as_ptr()
            },
            history_count: panel.history.len(),
        }
    }
}

fn native_text(text: &str) -> NativeText {
    NativeText {
        ptr: text.as_ptr(),
        len: text.len(),
    }
}

unsafe extern "C" {
    fn cclover_tui_enter(out: *mut *mut c_void) -> i32;
    fn cclover_tui_leave(ui: *mut c_void);
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

    pub(crate) fn draw(&mut self, frame: &Frame) -> io::Result<()> {
        let cpu = PanelView::new(&frame.cpu);
        let memory = PanelView::new(&frame.memory);
        let gpu = PanelView::new(&frame.gpu);
        let temperatures = PanelView::new(&frame.temperatures);
        let fans = PanelView::new(&frame.fans);
        let disks = PanelView::new(&frame.disks);
        let networks = PanelView::new(&frame.networks);
        let native = NativeFrame {
            cpu: cpu.native(&frame.cpu),
            memory: memory.native(&frame.memory),
            gpu: gpu.native(&frame.gpu),
            temperatures: temperatures.native(&frame.temperatures),
            fans: fans.native(&frame.fans),
            disks: disks.native(&frame.disks),
            networks: networks.native(&frame.networks),
        };
        // SAFETY: all pointers in `native` reference frame/view storage alive for this synchronous call.
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
