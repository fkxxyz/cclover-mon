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
}
