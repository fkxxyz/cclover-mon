#![allow(unsafe_code)]

use std::ffi::{c_char, c_int, c_void};
use std::mem;
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::{FreeLibrary, HMODULE};
use windows_sys::Win32::System::LibraryLoader::{
    GetProcAddress, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW,
};
use windows_sys::Win32::System::Memory::{GetProcessHeap, HeapAlloc};

const ADL_OK: c_int = 0;
const ADL_OK_WARNING: c_int = 1;
const ADL_MAX_PATH: usize = 256;
const ADL_PMLOG_MAX_SENSORS: usize = 256;
const PMLOG_CLK_GFXCLK: usize = 1;
const PMLOG_TEMPERATURE_EDGE: usize = 8;
const PMLOG_FAN_RPM: usize = 14;
const PMLOG_FAN_PERCENTAGE: usize = 15;
const PMLOG_INFO_ACTIVITY_GFX: usize = 19;
const PMLOG_ASIC_POWER: usize = 23;

// Public AMD ADL ABI. ADL functions use the C calling convention; only the allocator callback
// is __stdcall on 32-bit Windows (extern "system" here).
type AdlContext = *mut c_void;
type AdlMainMalloc = unsafe extern "system" fn(c_int) -> *mut c_void;
type Adl2MainControlCreate = unsafe extern "C" fn(AdlMainMalloc, c_int, *mut AdlContext) -> c_int;
type Adl2MainControlDestroy = unsafe extern "C" fn(AdlContext) -> c_int;
type Adl2AdapterNumberOfAdaptersGet = unsafe extern "C" fn(AdlContext, *mut c_int) -> c_int;
type Adl2AdapterAdapterInfoGet = unsafe extern "C" fn(AdlContext, *mut AdapterInfo, c_int) -> c_int;
type Adl2AdapterMemoryInfo2Get =
    unsafe extern "C" fn(AdlContext, c_int, *mut AdlMemoryInfo2) -> c_int;
type Adl2AdapterDedicatedVramUsageGet =
    unsafe extern "C" fn(AdlContext, c_int, *mut c_int) -> c_int;
type Adl2NewQueryPmLogDataGet =
    unsafe extern "C" fn(AdlContext, c_int, *mut AdlPmLogDataOutput) -> c_int;

#[repr(C)]
#[derive(Clone, Copy)]
struct AdapterInfo {
    size: c_int,
    adapter_index: c_int,
    udid: [c_char; ADL_MAX_PATH],
    bus_number: c_int,
    device_number: c_int,
    function_number: c_int,
    vendor_id: c_int,
    adapter_name: [c_char; ADL_MAX_PATH],
    display_name: [c_char; ADL_MAX_PATH],
    present: c_int,
    exist: c_int,
    driver_path: [c_char; ADL_MAX_PATH],
    driver_path_ext: [c_char; ADL_MAX_PATH],
    pnp_string: [c_char; ADL_MAX_PATH],
    os_display_index: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct AdlMemoryInfo2 {
    memory_size: i64,
    memory_type: [c_char; ADL_MAX_PATH],
    memory_bandwidth: i64,
    hyper_memory_size: i64,
    invisible_memory_size: i64,
    visible_memory_size: i64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct AdlSingleSensorData {
    supported: c_int,
    value: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct AdlPmLogDataOutput {
    size: c_int,
    sensors: [AdlSingleSensorData; ADL_PMLOG_MAX_SENSORS],
}

pub(super) struct Session {
    library: Library,
    context: AdlContext,
    destroy: Adl2MainControlDestroy,
    vram_usage: Option<Adl2AdapterDedicatedVramUsageGet>,
    pm_log: Option<Adl2NewQueryPmLogDataGet>,
    devices: Vec<Device>,
}

// SAFETY: ADL2 contexts support independent client transactions and are not thread-affine.
// cclover-mon moves the collector between threads but samples this Session serially.
unsafe impl Send for Session {}

pub(super) struct Device {
    adapter_index: c_int,
    stable_key: String,
    name: String,
    memory_total_bytes: Option<u64>,
}

pub(super) struct Telemetry {
    pub utilization_percent: Option<f64>,
    pub temperature_celsius: Option<f64>,
    pub power_watts: Option<f64>,
    pub core_clock_mhz: Option<u64>,
    pub fan_percent: Option<f64>,
    pub fan_rpm: Option<u64>,
}

impl Device {
    pub(super) fn stable_key(&self) -> &str {
        &self.stable_key
    }

    pub(super) fn name(&self) -> &str {
        &self.name
    }

    pub(super) fn memory_total_bytes(&self) -> Option<u64> {
        self.memory_total_bytes
    }
}

impl Session {
    pub(super) fn load() -> Result<(Self, Vec<String>), String> {
        let library = Library::open()?;
        let create: Adl2MainControlCreate = library.function(b"ADL2_Main_Control_Create\0")?;
        let destroy: Adl2MainControlDestroy = library.function(b"ADL2_Main_Control_Destroy\0")?;
        let get_count: Adl2AdapterNumberOfAdaptersGet =
            library.function(b"ADL2_Adapter_NumberOfAdapters_Get\0")?;
        let get_info: Adl2AdapterAdapterInfoGet =
            library.function(b"ADL2_Adapter_AdapterInfo_Get\0")?;
        let memory_info: Option<Adl2AdapterMemoryInfo2Get> =
            library.optional_function(b"ADL2_Adapter_MemoryInfo2_Get\0");
        let vram_usage: Option<Adl2AdapterDedicatedVramUsageGet> =
            library.optional_function(b"ADL2_Adapter_DedicatedVRAMUsage_Get\0");
        let pm_log: Option<Adl2NewQueryPmLogDataGet> =
            library.optional_function(b"ADL2_New_QueryPMLogData_Get\0");

        let mut context = null_mut();
        // SAFETY: callback and output pointer match the documented ADL2 ABI.
        let status = unsafe { create(adl_alloc, 1, &mut context) };
        if !adl_init_success(status) || context.is_null() {
            return Err(format!(
                "ADL2_Main_Control_Create failed with status {status}"
            ));
        }

        let mut count = 0;
        // SAFETY: context is live and count is writable.
        let status = unsafe { get_count(context, &mut count) };
        if status != ADL_OK || !(0..=250).contains(&count) {
            // SAFETY: context was successfully initialized above.
            unsafe { destroy(context) };
            return Err(format!(
                "ADL2_Adapter_NumberOfAdapters_Get failed with status {status}, count {count}"
            ));
        }

        let mut raw = vec![zeroed_adapter_info(); count as usize];
        let bytes = raw
            .len()
            .checked_mul(mem::size_of::<AdapterInfo>())
            .and_then(|size| c_int::try_from(size).ok())
            .ok_or_else(|| "ADL adapter list is too large".to_owned())?;
        if count > 0 {
            // SAFETY: raw is writable for exactly bytes bytes and context is live.
            let status = unsafe { get_info(context, raw.as_mut_ptr(), bytes) };
            if status != ADL_OK {
                // SAFETY: context was successfully initialized above.
                unsafe { destroy(context) };
                return Err(format!(
                    "ADL2_Adapter_AdapterInfo_Get failed with status {status}"
                ));
            }
        }

        let mut issues = Vec::new();
        if memory_info.is_none() {
            issues.push("ADL total VRAM export unavailable".to_owned());
        }
        if vram_usage.is_none() {
            issues.push("ADL dedicated VRAM usage export unavailable".to_owned());
        }
        if pm_log.is_none() {
            issues.push("ADL PMLog telemetry export unavailable".to_owned());
        }
        let mut devices = Vec::new();
        let mut physical_keys = std::collections::HashSet::new();
        for info in raw {
            if info.present == 0 || info.exist == 0 || !is_amd_adapter(&info) {
                continue;
            }
            let udid = c_string(&info.udid).filter(|value| !value.is_empty());
            let physical_key =
                if info.bus_number >= 0 && info.device_number >= 0 && info.function_number >= 0 {
                    format!(
                        "{:04x}:{:02x}:{:02x}.{}",
                        0_u16, info.bus_number, info.device_number, info.function_number
                    )
                } else if let Some(udid) = &udid {
                    format!("udid:{udid}")
                } else {
                    issues.push(format!(
                        "ADL adapter {} skipped: no stable UDID or PCI identity",
                        info.adapter_index
                    ));
                    continue;
                };
            if !physical_keys.insert(physical_key.clone()) {
                continue;
            }
            let stable_key = udid.unwrap_or(physical_key);
            let name = c_string(&info.adapter_name)
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| stable_key.clone());
            let memory_total_bytes = memory_info.and_then(|function| {
                let mut value = zeroed_memory_info();
                // SAFETY: context is live; value is writable and adapter index came from ADL.
                let status = unsafe { function(context, info.adapter_index, &mut value) };
                if status == ADL_OK && value.memory_size > 0 {
                    Some(value.memory_size as u64)
                } else {
                    issues.push(format!(
                        "ADL GPU {stable_key} total VRAM unavailable: status {status}"
                    ));
                    None
                }
            });
            devices.push(Device {
                adapter_index: info.adapter_index,
                stable_key,
                name,
                memory_total_bytes,
            });
        }

        Ok((
            Self {
                library,
                context,
                destroy,
                vram_usage,
                pm_log,
                devices,
            },
            issues,
        ))
    }

    pub(super) fn devices(&self) -> &[Device] {
        &self.devices
    }

    pub(super) fn memory_used_bytes(&self, index: usize) -> Result<Option<u64>, i32> {
        let Some(function) = self.vram_usage else {
            return Ok(None);
        };
        let device = self.devices.get(index).ok_or(i32::MIN)?;
        let mut megabytes = 0_i32;
        // SAFETY: context is live, adapter index came from ADL and output is writable.
        let status = unsafe { function(self.context, device.adapter_index, &mut megabytes) };
        if status != ADL_OK {
            return Err(status);
        }
        if megabytes < 0 {
            return Err(i32::MIN + 1);
        }
        Ok(Some((megabytes as u64).saturating_mul(1024 * 1024)))
    }

    pub(super) fn telemetry(&self, index: usize) -> Result<Option<Telemetry>, i32> {
        let Some(function) = self.pm_log else {
            return Ok(None);
        };
        let device = self.devices.get(index).ok_or(i32::MIN)?;
        let mut output = zeroed_pm_log();
        output.size = mem::size_of::<AdlPmLogDataOutput>() as c_int;
        // SAFETY: context is live, adapter index came from ADL and output is writable.
        let status = unsafe { function(self.context, device.adapter_index, &mut output) };
        if status != ADL_OK {
            return Err(status);
        }
        Ok(Some(Telemetry {
            utilization_percent: sensor_percent(&output, PMLOG_INFO_ACTIVITY_GFX),
            temperature_celsius: sensor_range(&output, PMLOG_TEMPERATURE_EDGE, -50, 150)
                .map(f64::from),
            power_watts: sensor_range(&output, PMLOG_ASIC_POWER, 0, 2_000).map(f64::from),
            core_clock_mhz: sensor_range(&output, PMLOG_CLK_GFXCLK, 0, 20_000)
                .map(|value| value as u64),
            fan_percent: sensor_percent(&output, PMLOG_FAN_PERCENTAGE),
            fan_rpm: sensor_range(&output, PMLOG_FAN_RPM, 0, 100_000).map(|value| value as u64),
        }))
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: context exists only after successful ADL2_Main_Control_Create.
        unsafe { (self.destroy)(self.context) };
        let _ = &self.library;
    }
}

unsafe extern "system" fn adl_alloc(size: c_int) -> *mut c_void {
    if size <= 0 {
        return null_mut();
    }
    // SAFETY: process heap is always valid for this process; ADL requests size bytes.
    unsafe { HeapAlloc(GetProcessHeap(), 0, size as usize) }
}

fn adl_init_success(status: i32) -> bool {
    status == ADL_OK || status == ADL_OK_WARNING
}

fn sensor_percent(output: &AdlPmLogDataOutput, index: usize) -> Option<f64> {
    sensor_range(output, index, 0, 100).map(f64::from)
}

fn sensor_range(
    output: &AdlPmLogDataOutput,
    index: usize,
    minimum: i32,
    maximum: i32,
) -> Option<i32> {
    let sensor = output.sensors.get(index)?;
    (sensor.supported != 0 && (minimum..=maximum).contains(&sensor.value)).then_some(sensor.value)
}

fn is_amd_adapter(info: &AdapterInfo) -> bool {
    c_string(&info.pnp_string).is_some_and(|value| value.to_ascii_uppercase().contains("VEN_1002"))
        || info.vendor_id == 0x1002
        || info.vendor_id == 1002
}

fn c_string<const N: usize>(value: &[c_char; N]) -> Option<String> {
    let len = value.iter().position(|byte| *byte == 0).unwrap_or(N);
    if len == 0 {
        return None;
    }
    let bytes = value[..len]
        .iter()
        .map(|byte| *byte as u8)
        .collect::<Vec<_>>();
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn zeroed_adapter_info() -> AdapterInfo {
    // SAFETY: AdapterInfo is a plain C POD structure for which all-zero is valid initialization.
    unsafe { mem::zeroed() }
}

fn zeroed_memory_info() -> AdlMemoryInfo2 {
    // SAFETY: AdlMemoryInfo2 is a plain C POD structure for which all-zero is valid initialization.
    unsafe { mem::zeroed() }
}

fn zeroed_pm_log() -> AdlPmLogDataOutput {
    // SAFETY: AdlPmLogDataOutput is a plain C POD structure for which all-zero is valid initialization.
    unsafe { mem::zeroed() }
}

struct Library {
    handle: HMODULE,
}

// SAFETY: an HMODULE is process-global and not thread-affine. Library uniquely owns the
// reference acquired by LoadLibraryExW and only exposes immutable symbol lookup.
unsafe impl Send for Library {}

impl Library {
    fn open() -> Result<Self, String> {
        #[cfg(target_pointer_width = "64")]
        let name = "atiadlxx.dll";
        #[cfg(target_pointer_width = "32")]
        let name = "atiadlxy.dll";
        let wide = wide_z(name);
        // SAFETY: name is NUL-terminated and search is constrained to System32/SysWOW64 redirection.
        let handle =
            unsafe { LoadLibraryExW(wide.as_ptr(), null_mut(), LOAD_LIBRARY_SEARCH_SYSTEM32) };
        if handle.is_null() {
            Err(format!("{name} not found in the Windows system directory"))
        } else {
            Ok(Self { handle })
        }
    }

    fn function<T: Copy>(&self, name: &'static [u8]) -> Result<T, String> {
        self.optional_function(name).ok_or_else(|| {
            format!(
                "ADL export {} is unavailable",
                String::from_utf8_lossy(&name[..name.len() - 1])
            )
        })
    }

    fn optional_function<T: Copy>(&self, name: &'static [u8]) -> Option<T> {
        debug_assert_eq!(name.last(), Some(&0));
        // SAFETY: handle is live and name is a NUL-terminated ASCII export name.
        let symbol = unsafe { GetProcAddress(self.handle, name.as_ptr()) }?;
        debug_assert_eq!(mem::size_of::<T>(), mem::size_of_val(&symbol));
        // SAFETY: callers request the concrete function-pointer type matching this export.
        Some(unsafe { mem::transmute_copy(&symbol) })
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        // SAFETY: handle was returned by LoadLibraryExW and is uniquely owned here.
        unsafe { FreeLibrary(self.handle) };
    }
}

fn wide_z(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_or_implausible_pm_log_values_are_absent() {
        let mut output = zeroed_pm_log();
        output.sensors[PMLOG_INFO_ACTIVITY_GFX] = AdlSingleSensorData {
            supported: 1,
            value: 67,
        };
        output.sensors[PMLOG_TEMPERATURE_EDGE] = AdlSingleSensorData {
            supported: 1,
            value: 65535,
        };
        assert_eq!(sensor_percent(&output, PMLOG_INFO_ACTIVITY_GFX), Some(67.0));
        assert_eq!(
            sensor_range(&output, PMLOG_TEMPERATURE_EDGE, -50, 150),
            None
        );
        assert_eq!(sensor_percent(&output, PMLOG_FAN_PERCENTAGE), None);
    }

    #[test]
    fn amd_adapter_detection_prefers_pnp_vendor_identity() {
        let mut info = zeroed_adapter_info();
        for (slot, byte) in info
            .pnp_string
            .iter_mut()
            .zip(b"PCI\\VEN_1002&DEV_73BF".iter().copied())
        {
            *slot = byte as c_char;
        }
        assert!(is_amd_adapter(&info));
    }
}
