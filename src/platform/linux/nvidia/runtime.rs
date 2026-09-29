#![allow(unsafe_code)]

use std::ffi::CStr;
use std::mem;

use libc::{RTLD_LOCAL, RTLD_NOW, c_char, c_uint, c_void};

const NVML_SUCCESS: c_uint = 0;
const NVML_TEMPERATURE_GPU: c_uint = 0;
const NVML_CLOCK_GRAPHICS: c_uint = 0;
const NVML_DEVICE_UUID_BUFFER_SIZE: usize = 96;
const NVML_DEVICE_NAME_BUFFER_SIZE: usize = 256;

type NvmlInit = unsafe extern "C" fn() -> c_uint;
type NvmlShutdown = unsafe extern "C" fn() -> c_uint;
type NvmlDeviceGetCount = unsafe extern "C" fn(*mut c_uint) -> c_uint;
type NvmlDeviceGetHandleByIndex = unsafe extern "C" fn(c_uint, *mut *mut c_void) -> c_uint;
type NvmlDeviceGetUuid = unsafe extern "C" fn(*mut c_void, *mut c_char, c_uint) -> c_uint;
type NvmlDeviceGetName = unsafe extern "C" fn(*mut c_void, *mut c_char, c_uint) -> c_uint;
type NvmlDeviceGetTemperature = unsafe extern "C" fn(*mut c_void, c_uint, *mut c_uint) -> c_uint;
type NvmlDeviceGetMemoryInfo = unsafe extern "C" fn(*mut c_void, *mut NvmlMemory) -> c_uint;
type NvmlDeviceGetUtilizationRates =
    unsafe extern "C" fn(*mut c_void, *mut NvmlUtilization) -> c_uint;
type NvmlDeviceGetPowerUsage = unsafe extern "C" fn(*mut c_void, *mut c_uint) -> c_uint;
type NvmlDeviceGetClockInfo = unsafe extern "C" fn(*mut c_void, c_uint, *mut c_uint) -> c_uint;
type NvmlDeviceGetFanSpeed = unsafe extern "C" fn(*mut c_void, *mut c_uint) -> c_uint;

#[repr(C)]
pub(super) struct NvmlMemory {
    pub(super) total: u64,
    pub(super) free: u64,
    pub(super) used: u64,
}

#[repr(C)]
pub(super) struct NvmlUtilization {
    pub(super) gpu: c_uint,
    pub(super) memory: c_uint,
}

pub(super) struct Session {
    library: Library,
    shutdown: NvmlShutdown,
    get_temperature: NvmlDeviceGetTemperature,
    get_memory_info: NvmlDeviceGetMemoryInfo,
    get_utilization_rates: NvmlDeviceGetUtilizationRates,
    get_power_usage: NvmlDeviceGetPowerUsage,
    get_clock_info: NvmlDeviceGetClockInfo,
    get_fan_speed: NvmlDeviceGetFanSpeed,
    devices: Vec<Device>,
}

pub(super) struct Device {
    handle: usize,
    uuid: String,
    name: String,
}

impl Device {
    pub(super) fn uuid(&self) -> &str {
        &self.uuid
    }

    pub(super) fn name(&self) -> &str {
        &self.name
    }
}

impl Session {
    pub(super) fn load() -> Result<(Self, Vec<String>), String> {
        let library = Library::open()?;
        let init: NvmlInit = library.function(b"nvmlInit_v2\0")?;
        let shutdown: NvmlShutdown = library.function(b"nvmlShutdown\0")?;
        let get_count: NvmlDeviceGetCount = library.function(b"nvmlDeviceGetCount_v2\0")?;
        let get_handle: NvmlDeviceGetHandleByIndex =
            library.function(b"nvmlDeviceGetHandleByIndex_v2\0")?;
        let get_uuid: NvmlDeviceGetUuid = library.function(b"nvmlDeviceGetUUID\0")?;
        let get_name: NvmlDeviceGetName = library.function(b"nvmlDeviceGetName\0")?;
        let get_temperature: NvmlDeviceGetTemperature =
            library.function(b"nvmlDeviceGetTemperature\0")?;
        let get_memory_info: NvmlDeviceGetMemoryInfo =
            library.function(b"nvmlDeviceGetMemoryInfo\0")?;
        let get_utilization_rates: NvmlDeviceGetUtilizationRates =
            library.function(b"nvmlDeviceGetUtilizationRates\0")?;
        let get_power_usage: NvmlDeviceGetPowerUsage =
            library.function(b"nvmlDeviceGetPowerUsage\0")?;
        let get_clock_info: NvmlDeviceGetClockInfo =
            library.function(b"nvmlDeviceGetClockInfo\0")?;
        let get_fan_speed: NvmlDeviceGetFanSpeed = library.function(b"nvmlDeviceGetFanSpeed\0")?;

        // SAFETY: init was resolved from the loaded NVML library with the exact NVML ABI signature.
        let init_status = unsafe { init() };
        if init_status != NVML_SUCCESS {
            return Err(format!("nvmlInit_v2 failed with status {init_status}"));
        }

        let mut count = 0_u32;
        // SAFETY: get_count was resolved with the exact NVML ABI signature and count is writable.
        let count_status = unsafe { get_count(&mut count) };
        if count_status != NVML_SUCCESS {
            // SAFETY: NVML initialization succeeded and shutdown matches that initialized library.
            unsafe { shutdown() };
            return Err(format!(
                "nvmlDeviceGetCount_v2 failed with status {count_status}"
            ));
        }

        let mut devices = Vec::with_capacity(count as usize);
        let mut issues = Vec::new();
        for index in 0..count {
            let mut handle = std::ptr::null_mut();
            // SAFETY: get_handle has the exact NVML ABI signature and handle points to writable storage.
            let handle_status = unsafe { get_handle(index, &mut handle) };
            if handle_status != NVML_SUCCESS || handle.is_null() {
                issues.push(format!(
                    "NVML device {index} skipped: handle query failed with status {handle_status}"
                ));
                continue;
            }

            let Some(uuid) = read_string::<NVML_DEVICE_UUID_BUFFER_SIZE>(get_uuid, handle) else {
                issues.push(format!("NVML device {index} skipped: UUID unavailable"));
                continue;
            };
            let name = read_string::<NVML_DEVICE_NAME_BUFFER_SIZE>(get_name, handle)
                .unwrap_or_else(|| {
                    issues.push(format!("NVML device {uuid}: name unavailable, using UUID"));
                    uuid.clone()
                });

            devices.push(Device {
                handle: handle as usize,
                uuid,
                name,
            });
        }

        Ok((
            Self {
                library,
                shutdown,
                get_temperature,
                get_memory_info,
                get_utilization_rates,
                get_power_usage,
                get_clock_info,
                get_fan_speed,
                devices,
            },
            issues,
        ))
    }

    pub(super) fn devices(&self) -> &[Device] {
        &self.devices
    }

    pub(super) fn temperature(&self, index: usize) -> Result<u32, u32> {
        let device = self.devices.get(index).ok_or(u32::MAX)?;
        let mut temperature = 0_u32;
        // SAFETY: get_temperature has the exact NVML ABI signature. The handle came from this
        // initialized session and remains valid while self owns the session.
        let status = unsafe {
            (self.get_temperature)(
                device.handle as *mut c_void,
                NVML_TEMPERATURE_GPU,
                &mut temperature,
            )
        };
        if status == NVML_SUCCESS {
            Ok(temperature)
        } else {
            Err(status)
        }
    }

    pub(super) fn memory(&self, index: usize) -> Result<NvmlMemory, u32> {
        let device = self.devices.get(index).ok_or(u32::MAX)?;
        let mut memory = NvmlMemory {
            total: 0,
            free: 0,
            used: 0,
        };
        // SAFETY: get_memory_info has the exact NVML ABI signature. The handle came from this
        // initialized session and memory points to writable storage.
        let status = unsafe { (self.get_memory_info)(device.handle as *mut c_void, &mut memory) };
        if status == NVML_SUCCESS {
            Ok(memory)
        } else {
            Err(status)
        }
    }

    pub(super) fn utilization(&self, index: usize) -> Result<NvmlUtilization, u32> {
        let device = self.devices.get(index).ok_or(u32::MAX)?;
        let mut utilization = NvmlUtilization { gpu: 0, memory: 0 };
        // SAFETY: get_utilization_rates has the exact NVML ABI signature. The handle came from
        // this initialized session and utilization points to writable storage.
        let status =
            unsafe { (self.get_utilization_rates)(device.handle as *mut c_void, &mut utilization) };
        if status == NVML_SUCCESS {
            Ok(utilization)
        } else {
            Err(status)
        }
    }

    pub(super) fn power_milliwatts(&self, index: usize) -> Result<u32, u32> {
        self.read_uint(index, self.get_power_usage)
    }

    pub(super) fn graphics_clock_mhz(&self, index: usize) -> Result<u32, u32> {
        self.read_clock(index, NVML_CLOCK_GRAPHICS)
    }

    pub(super) fn fan_percent(&self, index: usize) -> Result<u32, u32> {
        self.read_uint(index, self.get_fan_speed)
    }

    fn read_uint(
        &self,
        index: usize,
        function: unsafe extern "C" fn(*mut c_void, *mut c_uint) -> c_uint,
    ) -> Result<u32, u32> {
        let device = self.devices.get(index).ok_or(u32::MAX)?;
        let mut value = 0_u32;
        // SAFETY: function is one of the loaded NVML scalar getters with this exact signature;
        // the device handle is live and value points to writable storage.
        let status = unsafe { function(device.handle as *mut c_void, &mut value) };
        if status == NVML_SUCCESS {
            Ok(value)
        } else {
            Err(status)
        }
    }

    fn read_clock(&self, index: usize, clock: c_uint) -> Result<u32, u32> {
        let device = self.devices.get(index).ok_or(u32::MAX)?;
        let mut value = 0_u32;
        // SAFETY: get_clock_info has the exact NVML ABI signature. The handle came from this
        // initialized session and value points to writable storage.
        let status =
            unsafe { (self.get_clock_info)(device.handle as *mut c_void, clock, &mut value) };
        if status == NVML_SUCCESS {
            Ok(value)
        } else {
            Err(status)
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: this Session exists only after successful nvmlInit_v2 and owns that initialization.
        unsafe { (self.shutdown)() };
        // library drops after this method returns, so shutdown executes while symbols remain loaded.
        let _ = &self.library;
    }
}

fn read_string<const N: usize>(
    function: unsafe extern "C" fn(*mut c_void, *mut c_char, c_uint) -> c_uint,
    handle: *mut c_void,
) -> Option<String> {
    let mut buffer = [0_i8; N];
    // SAFETY: function was resolved with the exact NVML ABI signature, handle belongs to the live
    // session, and buffer provides N writable c_char elements.
    let status = unsafe { function(handle, buffer.as_mut_ptr(), N as c_uint) };
    if status != NVML_SUCCESS {
        return None;
    }
    // SAFETY: NVML string getters guarantee NUL termination on successful return for these buffers.
    let value = unsafe { CStr::from_ptr(buffer.as_ptr()) };
    value
        .to_str()
        .ok()
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

struct Library {
    handle: usize,
}

impl Library {
    fn open() -> Result<Self, String> {
        // SAFETY: the path is a static NUL-terminated string and flags are valid for dlopen.
        let handle = unsafe { libc::dlopen(c"libnvidia-ml.so.1".as_ptr(), RTLD_NOW | RTLD_LOCAL) };
        if handle.is_null() {
            return Err(last_dl_error());
        }
        Ok(Self {
            handle: handle as usize,
        })
    }

    fn function<T: Copy>(&self, name: &'static [u8]) -> Result<T, String> {
        debug_assert_eq!(name.last(), Some(&0));
        // SAFETY: dlerror has no preconditions; this clears any prior loader error.
        unsafe { libc::dlerror() };
        // SAFETY: self.handle is a live dlopen handle and name is NUL-terminated.
        let symbol = unsafe { libc::dlsym(self.handle as *mut c_void, name.as_ptr().cast()) };
        if symbol.is_null() {
            return Err(last_dl_error());
        }
        debug_assert_eq!(mem::size_of::<T>(), mem::size_of::<*mut c_void>());
        // SAFETY: callers request only the concrete NVML function-pointer type matching name.
        Ok(unsafe { mem::transmute_copy::<*mut c_void, T>(&symbol) })
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        // SAFETY: handle was returned by dlopen and this Library owns it exactly once.
        unsafe { libc::dlclose(self.handle as *mut c_void) };
    }
}

fn last_dl_error() -> String {
    // SAFETY: dlerror has no preconditions and returns null or a valid thread-local C string.
    let error = unsafe { libc::dlerror() };
    if error.is_null() {
        return "dynamic loader returned an unknown error".to_owned();
    }
    // SAFETY: a non-null dlerror result points to a NUL-terminated string valid until next loader call.
    unsafe { CStr::from_ptr(error) }
        .to_string_lossy()
        .into_owned()
}
