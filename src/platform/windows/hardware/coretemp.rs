#![allow(unsafe_code)]

use std::ffi::{CString, c_char, c_int, c_long, c_void};
use std::io;
use std::ptr::{NonNull, null_mut};

use super::super::pawnio::Session;
use super::cpu::{AffinityGuard, CoreAffinity, Info};

#[repr(C)]
struct Transport {
    context: *mut c_void,
    read_msr: Option<unsafe extern "C" fn(*mut c_void, u32, u32, *mut u64) -> c_int>,
}

#[repr(C)]
struct NativeState {
    _private: [u8; 0],
}

unsafe extern "C" {
    fn cclover_coretemp_create(
        family: u32,
        model: u32,
        stepping: u32,
        brand: *const c_char,
        transport: Transport,
        out_state: *mut *mut NativeState,
    ) -> c_int;
    fn cclover_coretemp_destroy(state: *mut NativeState);
    fn cclover_coretemp_read_core_millidegrees(
        state: *mut NativeState,
        cpu: u32,
        value: *mut c_long,
    ) -> c_int;
    fn cclover_coretemp_read_package_millidegrees(
        state: *mut NativeState,
        cpu: u32,
        value: *mut c_long,
    ) -> c_int;
}

struct Context {
    session: *const Session,
    cores: Vec<CoreAffinity>,
    error: Option<io::Error>,
}

pub(super) struct Collector {
    state: NonNull<NativeState>,
    context: Box<Context>,
    core_dts: bool,
    package_dts: bool,
    topology_degraded: bool,
}

pub(super) struct Observation {
    pub core_temperatures: Vec<(usize, f64)>,
    pub package_temperature: Option<f64>,
    pub degraded: bool,
    pub core_count: usize,
    pub affinity_failures: usize,
    pub invalid_core_dts: usize,
}

// SAFETY: Collector owns the native state and heap-stable callback context. It is movable between
// threads but not Sync; all native calls require &mut self, and the native active-state slot is TLS.
unsafe impl Send for Collector {}

impl Collector {
    pub(super) fn new(info: &Info) -> io::Result<Self> {
        let topology = if info.core_dts || info.package_dts {
            super::cpu::physical_core_affinities()
        } else {
            Ok(Vec::new())
        };
        let (cores, topology_degraded) = match topology {
            Ok(cores) => (cores, false),
            Err(_) => (Vec::new(), true),
        };

        let brand = CString::new(info.brand.as_bytes()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "CPU brand contains an embedded NUL",
            )
        })?;
        let mut context = Box::new(Context {
            session: std::ptr::null(),
            cores,
            error: None,
        });
        let transport = Transport {
            context: (&mut *context as *mut Context).cast(),
            read_msr: Some(read_msr),
        };
        let mut state = null_mut();
        // SAFETY: brand is NUL-terminated for this call; context remains heap-stable for Collector's
        // lifetime; out_state points to writable storage and native retains only the context pointer.
        let status = unsafe {
            cclover_coretemp_create(
                info.family,
                info.model,
                info.stepping,
                brand.as_ptr(),
                transport,
                &mut state,
            )
        };
        if status != 0 {
            return Err(io::Error::from_raw_os_error(-status));
        }
        let state = NonNull::new(state)
            .ok_or_else(|| io::Error::other("coretemp compatibility facade returned null state"))?;

        Ok(Self {
            state,
            context,
            core_dts: info.core_dts,
            package_dts: info.package_dts,
            topology_degraded,
        })
    }

    pub(super) fn collect(&mut self, session: &Session) -> io::Result<Observation> {
        self.context.session = session;
        self.context.error = None;
        let mut degraded = self.topology_degraded;
        let mut affinity_failures = 0;
        let mut invalid_core_dts = 0;
        let mut core_temperatures = Vec::new();

        if self.core_dts {
            if self.context.cores.is_empty() {
                degraded = true;
            }
            for cpu in 0..self.context.cores.len() {
                match self.read(false, cpu as u32) {
                    Ok(celsius) if (0.0..=125.0).contains(&celsius) => {
                        core_temperatures.push((cpu, celsius));
                    }
                    Ok(_) => {
                        degraded = true;
                        invalid_core_dts += 1;
                    }
                    Err(error) if error.kind() == io::ErrorKind::InvalidInput => {
                        degraded = true;
                        affinity_failures += 1;
                    }
                    Err(error) => return Err(error),
                }
            }
        }

        let package_temperature = if self.package_dts {
            match self.read(true, 0) {
                Ok(celsius) if (0.0..=125.0).contains(&celsius) => Some(celsius),
                Ok(_) => {
                    degraded = true;
                    None
                }
                Err(error) => return Err(error),
            }
        } else {
            None
        };

        Ok(Observation {
            core_temperatures,
            package_temperature,
            degraded,
            core_count: self.context.cores.len(),
            affinity_failures,
            invalid_core_dts,
        })
    }

    fn read(&mut self, package: bool, cpu: u32) -> io::Result<f64> {
        self.context.error = None;
        let mut millidegrees: c_long = 0;
        // SAFETY: state is owned by self and millidegrees points to writable storage for this call.
        let status = unsafe {
            if package {
                cclover_coretemp_read_package_millidegrees(
                    self.state.as_ptr(),
                    cpu,
                    &mut millidegrees,
                )
            } else {
                cclover_coretemp_read_core_millidegrees(self.state.as_ptr(), cpu, &mut millidegrees)
            }
        };
        if status != 0 {
            if let Some(error) = self.context.error.take() {
                return Err(error);
            }
            return Err(io::Error::from_raw_os_error(-status));
        }
        Ok(millidegrees as f64 / 1000.0)
    }
}

impl Drop for Collector {
    fn drop(&mut self) {
        // SAFETY: state was returned by cclover_coretemp_create and is destroyed exactly once.
        unsafe { cclover_coretemp_destroy(self.state.as_ptr()) };
    }
}

unsafe extern "C" fn read_msr(context: *mut c_void, cpu: u32, msr: u32, value: *mut u64) -> c_int {
    if context.is_null() || value.is_null() {
        return -22;
    }
    // SAFETY: context points to Collector-owned Context for the entire native call.
    let context = unsafe { &mut *context.cast::<Context>() };
    if context.session.is_null() {
        return -5;
    }

    let affinity = context.cores.get(cpu as usize).copied();
    let _guard = match affinity.map(AffinityGuard::bind).transpose() {
        Ok(guard) => guard,
        Err(error) => {
            context.error = Some(io::Error::new(io::ErrorKind::InvalidInput, error));
            return -22;
        }
    };

    // SAFETY: session is refreshed from a live shared reference immediately before every facade call.
    let session = unsafe { &*context.session };
    match session
        .execute("ioctl_read_msr", &[u64::from(msr)], 1)
        .and_then(|output| {
            output.first().copied().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "PawnIO MSR read returned no value",
                )
            })
        }) {
        Ok(raw) => {
            // SAFETY: checked non-null above.
            unsafe { *value = raw };
            0
        }
        Err(error) => {
            let code = error.raw_os_error().unwrap_or(5).abs().max(1);
            context.error = Some(error);
            -code
        }
    }
}
