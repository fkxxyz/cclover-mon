#![allow(unsafe_code)]

use std::ffi::{CStr, c_char, c_uint, c_void};
use std::mem;
use std::path::PathBuf;
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::{FreeLibrary, HMODULE};
use windows_sys::Win32::System::LibraryLoader::{
    GetProcAddress, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW,
};

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

// SAFETY: NVML device handles and the loaded module are not thread-affine. Session has unique
// ownership, and cclover-mon moves the collector between threads but samples it serially.
unsafe impl Send for Session {}

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

        // SAFETY: symbol has the exact NVML ABI signature and remains loaded through library.
        let init_status = unsafe { init() };
        if init_status != NVML_SUCCESS {
            return Err(format!("nvmlInit_v2 failed with status {init_status}"));
        }

        let mut count = 0_u32;
        // SAFETY: count is writable and symbol has the exact NVML ABI signature.
        let count_status = unsafe { get_count(&mut count) };
        if count_status != NVML_SUCCESS {
            // SAFETY: initialization succeeded, so the matching shutdown is valid here.
            unsafe { shutdown() };
            return Err(format!(
                "nvmlDeviceGetCount_v2 failed with status {count_status}"
            ));
        }

        let mut devices = Vec::with_capacity(count as usize);
        let mut issues = Vec::new();
        for index in 0..count {
            let mut handle = null_mut();
            // SAFETY: handle points to writable storage and get_handle uses the exact NVML ABI.
            let status = unsafe { get_handle(index, &mut handle) };
            if status != NVML_SUCCESS || handle.is_null() {
                issues.push(format!(
                    "NVML device {index} skipped: handle query failed with status {status}"
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
        let mut value = 0_u32;
        // SAFETY: device handle belongs to this live session and value is writable.
        let status = unsafe {
            (self.get_temperature)(
                device.handle as *mut c_void,
                NVML_TEMPERATURE_GPU,
                &mut value,
            )
        };
        result(status, value)
    }

    pub(super) fn memory(&self, index: usize) -> Result<NvmlMemory, u32> {
        let device = self.devices.get(index).ok_or(u32::MAX)?;
        let mut value = NvmlMemory {
            total: 0,
            free: 0,
            used: 0,
        };
        // SAFETY: device handle belongs to this live session and value is writable.
        let status = unsafe { (self.get_memory_info)(device.handle as *mut c_void, &mut value) };
        if status == NVML_SUCCESS {
            Ok(value)
        } else {
            Err(status)
        }
    }

    pub(super) fn utilization(&self, index: usize) -> Result<NvmlUtilization, u32> {
        let device = self.devices.get(index).ok_or(u32::MAX)?;
        let mut value = NvmlUtilization { gpu: 0, memory: 0 };
        // SAFETY: device handle belongs to this live session and value is writable.
        let status =
            unsafe { (self.get_utilization_rates)(device.handle as *mut c_void, &mut value) };
        if status == NVML_SUCCESS {
            Ok(value)
        } else {
            Err(status)
        }
    }

    pub(super) fn power_milliwatts(&self, index: usize) -> Result<u32, u32> {
        self.read_uint(index, self.get_power_usage)
    }

    pub(super) fn graphics_clock_mhz(&self, index: usize) -> Result<u32, u32> {
        let device = self.devices.get(index).ok_or(u32::MAX)?;
        let mut value = 0_u32;
        // SAFETY: device handle belongs to this live session and value is writable.
        let status = unsafe {
            (self.get_clock_info)(
                device.handle as *mut c_void,
                NVML_CLOCK_GRAPHICS,
                &mut value,
            )
        };
        result(status, value)
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
        // SAFETY: function is an NVML scalar getter and device/value are valid.
        let status = unsafe { function(device.handle as *mut c_void, &mut value) };
        result(status, value)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: this session exists only after successful nvmlInit_v2.
        unsafe { (self.shutdown)() };
        let _ = &self.library;
    }
}

fn result(status: u32, value: u32) -> Result<u32, u32> {
    if status == NVML_SUCCESS {
        Ok(value)
    } else {
        Err(status)
    }
}

fn read_string<const N: usize>(
    function: unsafe extern "C" fn(*mut c_void, *mut c_char, c_uint) -> c_uint,
    handle: *mut c_void,
) -> Option<String> {
    let mut buffer = [0_i8; N];
    // SAFETY: handle belongs to the initialized session and buffer is writable for N chars.
    let status = unsafe { function(handle, buffer.as_mut_ptr(), N as u32) };
    if status != NVML_SUCCESS {
        return None;
    }
    // SAFETY: successful NVML string getters NUL-terminate these output buffers.
    unsafe { CStr::from_ptr(buffer.as_ptr()) }
        .to_str()
        .ok()
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

struct Library {
    handle: HMODULE,
}

// SAFETY: an HMODULE is process-global and not thread-affine. Library uniquely owns
// the reference acquired by LoadLibraryExW and only exposes immutable symbol lookup.
unsafe impl Send for Library {}

impl Library {
    fn open() -> Result<Self, String> {
        if let Some(handle) = load_system32() {
            return Ok(Self { handle });
        }
        if let Some(handle) = load_legacy_path() {
            return Ok(Self { handle });
        }
        Err("nvml.dll not found in System32 or NVIDIA NVSMI directory".to_owned())
    }

    fn function<T: Copy>(&self, name: &'static [u8]) -> Result<T, String> {
        debug_assert_eq!(name.last(), Some(&0));
        // SAFETY: handle is live and name is a NUL-terminated ASCII export name.
        let symbol = unsafe { GetProcAddress(self.handle, name.as_ptr()) };
        let Some(symbol) = symbol else {
            return Err(format!(
                "NVML export {} is unavailable",
                String::from_utf8_lossy(&name[..name.len() - 1])
            ));
        };
        debug_assert_eq!(mem::size_of::<T>(), mem::size_of_val(&symbol));
        // SAFETY: callers request the concrete function-pointer type matching this export.
        Ok(unsafe { mem::transmute_copy(&symbol) })
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        // SAFETY: handle was returned by LoadLibraryExW and is uniquely owned here.
        unsafe { FreeLibrary(self.handle) };
    }
}

fn load_system32() -> Option<HMODULE> {
    let name = wide_z("nvml.dll");
    // SAFETY: path is NUL-terminated; null file handle and documented search flag are valid.
    let handle = unsafe { LoadLibraryExW(name.as_ptr(), null_mut(), LOAD_LIBRARY_SEARCH_SYSTEM32) };
    (!handle.is_null()).then_some(handle)
}

fn load_legacy_path() -> Option<HMODULE> {
    let root = std::env::var_os("ProgramW6432")?;
    let path = PathBuf::from(root)
        .join("NVIDIA Corporation")
        .join("NVSMI")
        .join("nvml.dll");
    let wide = wide_path(&path);
    // SAFETY: absolute path is NUL-terminated; no search flags are needed for the full path.
    let handle = unsafe { LoadLibraryExW(wide.as_ptr(), null_mut(), 0) };
    (!handle.is_null()).then_some(handle)
}

fn wide_z(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn wide_path(path: &std::path::Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}
