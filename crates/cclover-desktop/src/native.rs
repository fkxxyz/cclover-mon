#![allow(unsafe_code)]

use std::ffi::c_void;
use std::sync::{Arc, Mutex};

use cclover_core::model::MonitorState;
mod abi;
mod frame;
mod state;

pub(crate) use abi::*;
use frame::{FrameStorage, build_frame};
pub use state::DesktopApp;
pub(crate) use state::NativeStateBridge;

pub(crate) struct NativeContext {
    pending: Arc<Mutex<Option<MonitorState>>>,
    state: MonitorState,
    frame: Option<FrameStorage>,
    frame_dirty: bool,
    static_revision: u64,
}

impl NativeContext {
    pub(crate) fn new(pending: Arc<Mutex<Option<MonitorState>>>) -> Self {
        Self {
            pending,
            state: MonitorState::default(),
            frame: None,
            frame_dirty: true,
            static_revision: 0,
        }
    }

    fn take_state(&mut self) -> u32 {
        let latest = self
            .pending
            .lock()
            .expect("native pending state lock poisoned")
            .take();
        if let Some(state) = latest {
            self.state = state;
            self.frame_dirty = true;
            return STATE_CHANGED;
        }
        0
    }

    pub(crate) fn as_ptr(&mut self) -> *mut c_void {
        (self as *mut Self).cast::<c_void>()
    }
}

impl HostCallbacks {
    pub(crate) const fn new() -> Self {
        Self {
            take_state: take_state_callback,
            scene: scene_callback,
        }
    }
}

unsafe extern "C" fn take_state_callback(context: *mut c_void) -> u32 {
    // SAFETY: platform hosts receive this pointer from `run` and use it only synchronously.
    let context = unsafe { &mut *(context.cast::<NativeContext>()) };
    context.take_state()
}

unsafe extern "C" fn scene_callback(context: *mut c_void, scene: *mut SceneView) {
    // SAFETY: pointers and callback are supplied by the synchronous native host contract.
    let context = unsafe { &mut *(context.cast::<NativeContext>()) };
    if context.frame_dirty || context.frame.is_none() {
        let frame = build_frame(
            &context.state,
            context.frame.as_ref().map(FrameStorage::scene),
            context.static_revision,
        );
        context.static_revision = frame.static_revision();
        context.frame = Some(frame);
        context.frame_dirty = false;
    }
    let frame = context
        .frame
        .as_ref()
        .expect("scene frame must be initialized");
    unsafe { scene.write(frame.view()) };
}

#[cfg(test)]
mod tests {
    use crossbeam_channel::unbounded;

    use super::*;

    #[cfg(target_os = "windows")]
    unsafe extern "C" {
        fn cclover_gdi_validate_scene(dc: *mut c_void, scene: *const SceneView, dpi: u32) -> i32;
    }

    #[cfg(target_os = "windows")]
    #[allow(non_snake_case)]
    unsafe extern "system" {
        fn CreateCompatibleDC(dc: *mut c_void) -> *mut c_void;
        fn DeleteDC(dc: *mut c_void) -> i32;
    }

    #[cfg(target_os = "linux")]
    unsafe extern "C" {
        fn cairo_image_surface_create(format: i32, width: i32, height: i32) -> *mut c_void;
        fn cairo_surface_destroy(surface: *mut c_void);
        fn cairo_create(surface: *mut c_void) -> *mut c_void;
        fn cairo_destroy(cr: *mut c_void);
        fn cclover_cairo_configure_context(cr: *mut c_void);
        fn cclover_cairo_validate_scene(cr: *mut c_void, scene: *const SceneView, mode: i32)
        -> i32;
    }

    #[test]
    fn publication_wakes_and_latest_state_requests_a_new_frame() {
        let (sender, receiver) = unbounded();
        let (wake_sender, wake_receiver) = unbounded();
        let bridge = NativeStateBridge::spawn(receiver, move || {
            wake_sender
                .send(())
                .expect("test wake receiver must remain connected");
        });

        for history_capacity in 1..=3 {
            sender
                .send(MonitorState {
                    history_capacity,
                    ..MonitorState::default()
                })
                .expect("state bridge receiver must remain connected");
        }
        for _ in 0..3 {
            wake_receiver
                .recv()
                .expect("every publication must wake the native host");
        }

        let mut context = NativeContext::new(bridge.pending());
        context.frame_dirty = false;
        assert_eq!(context.take_state(), STATE_CHANGED);
        assert_eq!(context.state.history_capacity, 3);
        assert!(context.frame_dirty);
        assert_eq!(
            context.take_state(),
            0,
            "pending state must be consumed once"
        );

        drop(bridge);
        assert!(sender.send(MonitorState::default()).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn production_cairo_font_realization_satisfies_scene_slots_and_rejects_overflow() {
        const CAIRO_FORMAT_ARGB32: i32 = 0;
        const CCLOVER_CAIRO_DRAW_ALL: i32 = 0;

        let surface = unsafe { cairo_image_surface_create(CAIRO_FORMAT_ARGB32, 1, 1) };
        assert!(!surface.is_null(), "Cairo test surface must be available");
        let cr = unsafe { cairo_create(surface) };
        assert!(!cr.is_null(), "Cairo test context must be available");
        unsafe { cclover_cairo_configure_context(cr) };

        let frame = build_frame(&MonitorState::default(), None, 0);
        let production = frame.view();
        assert_eq!(
            unsafe { cclover_cairo_validate_scene(cr, &production, CCLOVER_CAIRO_DRAW_ALL) },
            1,
            "production bounded text must fit the actual Cairo font realization"
        );

        let text = b"999.9%";
        let command = NativeCommand {
            kind: COMMAND_TEXT,
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 20.0,
            color: 0xffffffff,
            radius: 0.0,
            stroke_width: 0.0,
            point_offset: 0,
            point_count: 0,
            text: text.as_ptr(),
            text_len: text.len(),
            text_size: 16,
            flags: FLAG_TEXT_BOLD | FLAG_TEXT_END | FLAG_TEXT_MUST_FIT,
        };
        let overflow = SceneView {
            width: 100,
            height: 40,
            static_revision: 1,
            full_redraw: 1,
            damage_rects: std::ptr::null(),
            damage_count: 0,
            redraw_mask: std::ptr::null(),
            redraw_mask_count: 0,
            commands: &command,
            command_count: 1,
            points: std::ptr::null(),
            point_count: 0,
        };
        assert_eq!(
            unsafe { cclover_cairo_validate_scene(cr, &overflow, CCLOVER_CAIRO_DRAW_ALL) },
            0,
            "Cairo must reject a MustFit string wider than its authoritative Scene slot"
        );

        unsafe {
            cairo_destroy(cr);
            cairo_surface_destroy(surface);
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn production_gdi_font_realization_satisfies_scene_slots_and_rejects_overflow() {
        let dc = unsafe { CreateCompatibleDC(std::ptr::null_mut()) };
        assert!(!dc.is_null(), "GDI test device context must be available");

        let frame = build_frame(&MonitorState::default(), None, 0);
        let production = frame.view();
        assert_eq!(
            unsafe { cclover_gdi_validate_scene(dc, &production, 96) },
            1,
            "production bounded text must fit the actual GDI font realization at 96 DPI"
        );

        let text = b"999.9%";
        let command = NativeCommand {
            kind: COMMAND_TEXT,
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 20.0,
            color: 0xffffffff,
            radius: 0.0,
            stroke_width: 0.0,
            point_offset: 0,
            point_count: 0,
            text: text.as_ptr(),
            text_len: text.len(),
            text_size: 16,
            flags: FLAG_TEXT_BOLD | FLAG_TEXT_END | FLAG_TEXT_MUST_FIT,
        };
        let overflow = SceneView {
            width: 100,
            height: 40,
            static_revision: 1,
            full_redraw: 1,
            damage_rects: std::ptr::null(),
            damage_count: 0,
            redraw_mask: std::ptr::null(),
            redraw_mask_count: 0,
            commands: &command,
            command_count: 1,
            points: std::ptr::null(),
            point_count: 0,
        };
        assert_eq!(
            unsafe { cclover_gdi_validate_scene(dc, &overflow, 96) },
            0,
            "GDI must reject a MustFit string wider than its authoritative Scene slot"
        );

        unsafe { DeleteDC(dc) };
    }
}
