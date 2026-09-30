#![allow(unsafe_code)]

use std::ffi::{CString, c_char, c_int, c_long, c_void};
use std::io;
use std::ptr::{NonNull, null_mut};

use super::super::pawnio::Session;
use super::cpu::Info;

#[repr(C)]
struct Transport {
    context: *mut c_void,
    read_smn: Option<unsafe extern "C" fn(*mut c_void, u32, *mut u32) -> c_int>,
    read_pci: Option<unsafe extern "C" fn(*mut c_void, u32, *mut u32) -> c_int>,
    read_indexed: Option<unsafe extern "C" fn(*mut c_void, u32, *mut u32) -> c_int>,
}

#[repr(C)]
struct NativeState {
    _private: [u8; 0],
}

unsafe extern "C" {
    fn cclover_k10_create(
        family: u32,
        model: u32,
        stepping: u32,
        brand: *const c_char,
        transport: Transport,
        out_state: *mut *mut NativeState,
    ) -> c_int;
    fn cclover_k10_destroy(state: *mut NativeState);
    fn cclover_k10_channel_visible(state: *mut NativeState, channel: u32) -> c_int;
    fn cclover_k10_read_millidegrees(
        state: *mut NativeState,
        channel: u32,
        value: *mut c_long,
    ) -> c_int;
    fn cclover_k10_max_channels() -> u32;
}

struct Context {
    session: *const Session,
    error: Option<io::Error>,
}

pub(super) struct Collector {
    state: NonNull<NativeState>,
    context: Box<Context>,
}

// SAFETY: Collector owns the native state and heap-stable callback context. It is movable between
// threads but not Sync; all facade calls require &mut self, and the native active-state slot is TLS.
unsafe impl Send for Collector {}

impl Collector {
    pub(super) fn new(info: &Info, session: &Session) -> io::Result<Self> {
        let brand = CString::new(info.brand.as_bytes()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "CPU brand contains an embedded NUL",
            )
        })?;
        let mut context = Box::new(Context {
            session,
            error: None,
        });
        let transport = Transport {
            context: (&mut *context as *mut Context).cast(),
            read_smn: Some(read_smn),
            read_pci: Some(read_pci),
            read_indexed: Some(read_indexed),
        };
        let mut state = null_mut();
        // SAFETY: brand is NUL-terminated for this call; context remains heap-stable for Collector's lifetime;
        // out_state points to writable storage and the native side retains only the context pointer.
        let status = unsafe {
            cclover_k10_create(
                info.family,
                info.model,
                info.stepping,
                brand.as_ptr(),
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
            .ok_or_else(|| io::Error::other("k10temp compatibility facade returned null state"))?;
        Ok(Self { state, context })
    }

    pub(super) fn read_channels(&mut self, session: &Session) -> io::Result<Vec<Option<f64>>> {
        self.context.session = session;
        self.context.error = None;
        // SAFETY: state is owned by self and remains valid until Drop.
        let count = unsafe { cclover_k10_max_channels() };
        let mut values = Vec::with_capacity(count as usize);
        for channel in 0..count {
            // SAFETY: state is valid and channel is bounded by the facade-reported channel count.
            if unsafe { cclover_k10_channel_visible(self.state.as_ptr(), channel) } == 0 {
                values.push(None);
                continue;
            }
            let mut millidegrees: c_long = 0;
            // SAFETY: state is valid and millidegrees points to writable storage for this call.
            let status = unsafe {
                cclover_k10_read_millidegrees(self.state.as_ptr(), channel, &mut millidegrees)
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
        // SAFETY: state was returned by cclover_k10_create and is destroyed exactly once.
        unsafe { cclover_k10_destroy(self.state.as_ptr()) };
    }
}

unsafe extern "C" fn read_smn(context: *mut c_void, address: u32, value: *mut u32) -> c_int {
    read_u32_command(context, value, "ioctl_read_smn", &[u64::from(address)])
}

unsafe extern "C" fn read_pci(context: *mut c_void, offset: u32, value: *mut u32) -> c_int {
    read_u32_command(
        context,
        value,
        "ioctl_read_miscctl",
        &[0, u64::from(offset)],
    )
}

unsafe extern "C" fn read_indexed(context: *mut c_void, address: u32, value: *mut u32) -> c_int {
    read_u32_command(context, value, "ioctl_read_smu", &[u64::from(address)])
}

fn read_u32_command(context: *mut c_void, value: *mut u32, command: &str, input: &[u64]) -> c_int {
    if context.is_null() || value.is_null() {
        return -22;
    }
    // SAFETY: context points to Collector-owned Context for the entire native call.
    let context = unsafe { &mut *context.cast::<Context>() };
    if context.session.is_null() {
        return -5;
    }
    // SAFETY: session is refreshed from a live shared reference immediately before every facade call.
    let session = unsafe { &*context.session };
    match session.execute(command, input, 1).and_then(|output| {
        output.first().copied().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("PawnIO {command} returned no value"),
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
