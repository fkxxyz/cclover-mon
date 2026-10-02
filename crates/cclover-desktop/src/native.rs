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
