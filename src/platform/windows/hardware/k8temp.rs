#![allow(unsafe_code)]

use std::ffi::{c_int, c_long, c_void};
use std::io;
use std::ptr::{NonNull, null_mut};

use super::super::pawnio::Session;
use super::cpu::Info;

#[repr(C)]
struct Transport {
    context: *mut c_void,
    read_thermtrip: Option<unsafe extern "C" fn(*mut c_void, u32, *mut u32) -> c_int>,
}

#[repr(C)]
struct NativeState {
    _private: [u8; 0],
}

unsafe extern "C" {
    fn cclover_k8_create(
        family: u32,
        model: u32,
        stepping: u32,
        cpuid_80000001_ebx: u32,
        transport: Transport,
        out_state: *mut *mut NativeState,
    ) -> c_int;
    fn cclover_k8_destroy(state: *mut NativeState);
    fn cclover_k8_channel_visible(state: *mut NativeState, channel: u32) -> c_int;
    fn cclover_k8_read_millidegrees(
        state: *mut NativeState,
        channel: u32,
        value: *mut c_long,
    ) -> c_int;
    fn cclover_k8_max_channels() -> u32;
}

struct Context {
    session: *const Session,
    error: Option<io::Error>,
}

pub(super) struct Collector {
    state: NonNull<NativeState>,
    context: Box<Context>,
}

// SAFETY: native state and callback context are exclusively owned by Collector; facade access is
// serialized by &mut self and the native active-state pointer is thread-local.
unsafe impl Send for Collector {}

impl Collector {
    pub(super) fn new(info: &Info, session: &Session) -> io::Result<Self> {
        let mut context = Box::new(Context {
            session,
            error: None,
        });
        let transport = Transport {
            context: (&mut *context as *mut Context).cast(),
            read_thermtrip: Some(read_thermtrip),
        };
        let mut state = null_mut();
        // SAFETY: context remains heap-stable for Collector's lifetime and out_state is writable.
        let status = unsafe {
            cclover_k8_create(
                info.family,
                info.model,
                info.stepping,
                info.cpuid_80000001_ebx,
                transport,
                &mut state,
            )
        };
        if status != 0 {
            if let Some(error) = context.error.take() {
                return Err(error);
            }
            return Err(io::Error::from_raw_os_error(-status));
        }
        let state = NonNull::new(state)
            .ok_or_else(|| io::Error::other("k8temp compatibility facade returned null state"))?;
        Ok(Self { state, context })
    }

    pub(super) fn read_channels(&mut self, session: &Session) -> io::Result<Vec<Option<f64>>> {
        self.context.session = session;
        self.context.error = None;
        // SAFETY: state is valid until Drop.
        let count = unsafe { cclover_k8_max_channels() };
        let mut values = Vec::with_capacity(count as usize);
        for channel in 0..count {
            // SAFETY: channel is bounded by facade-reported count.
            if unsafe { cclover_k8_channel_visible(self.state.as_ptr(), channel) } == 0 {
                values.push(None);
                continue;
            }
            let mut millidegrees = 0 as c_long;
            // SAFETY: state is valid and output points to writable storage.
            let status = unsafe {
                cclover_k8_read_millidegrees(self.state.as_ptr(), channel, &mut millidegrees)
            };
            if status != 0 {
                if let Some(error) = self.context.error.take() {
                    return Err(error);
                }
                return Err(io::Error::from_raw_os_error(-status));
            }
            values.push(Some(millidegrees as f64 / 1000.0));
        }
        Ok(values)
    }
}

impl Drop for Collector {
    fn drop(&mut self) {
        // SAFETY: state was returned by cclover_k8_create and is destroyed exactly once.
        unsafe { cclover_k8_destroy(self.state.as_ptr()) };
    }
}

unsafe extern "C" fn read_thermtrip(context: *mut c_void, core: u32, value: *mut u32) -> c_int {
    if context.is_null() || value.is_null() || core > 1 {
        return -22;
    }
    // SAFETY: context is Collector-owned and heap-stable for the native call.
    let context = unsafe { &mut *context.cast::<Context>() };
    if context.session.is_null() {
        return -5;
    }
    // SAFETY: refreshed from a live shared reference immediately before facade calls.
    let session = unsafe { &*context.session };
    match session
        .execute("ioctl_get_thermtrip", &[0, u64::from(core)], 1)
        .and_then(|output| {
            output.first().copied().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "PawnIO ioctl_get_thermtrip returned no value",
                )
            })
        }) {
        Ok(raw) => {
            // SAFETY: checked non-null above.
            unsafe { *value = raw as u32 };
            0
        }
        Err(error) => {
            let code = error.raw_os_error().unwrap_or(5).abs().max(1);
            context.error = Some(error);
            -code
        }
    }
}
