use std::ffi::CStr;
use std::mem;

use libc::{RTLD_LOCAL, RTLD_NOW, c_char, c_uint, c_void};

use crate::core::model::TemperatureSnapshot;

use super::super::diagnostics::{probe_note, report_issue};

const NVML_SUCCESS: c_uint = 0;
const NVML_TEMPERATURE_GPU: c_uint = 0;
const NVML_DEVICE_UUID_BUFFER_SIZE: usize = 96;
const NVML_DEVICE_NAME_BUFFER_SIZE: usize = 256;

type NvmlInit = unsafe extern "C" fn() -> c_uint;
type NvmlShutdown = unsafe extern "C" fn() -> c_uint;
type NvmlDeviceGetCount = unsafe extern "C" fn(*mut c_uint) -> c_uint;
type NvmlDeviceGetHandleByIndex = unsafe extern "C" fn(c_uint, *mut *mut c_void) -> c_uint;
type NvmlDeviceGetUuid = unsafe extern "C" fn(*mut c_void, *mut c_char, c_uint) -> c_uint;
type NvmlDeviceGetName = unsafe extern "C" fn(*mut c_void, *mut c_char, c_uint) -> c_uint;
type NvmlDeviceGetTemperature = unsafe extern "C" fn(*mut c_void, c_uint, *mut c_uint) -> c_uint;

pub(super) struct Collector {
    state: State,
}

enum State {
    Uninitialized,
    Available(Session),
    Unavailable(String),
}

struct Session {
    library: usize,
    shutdown: NvmlShutdown,
    get_temperature: NvmlDeviceGetTemperature,
    devices: Vec<Device>,
}

struct Device {
    handle: usize,
    uuid: String,
    name: String,
}

impl Collector {
    pub(super) fn new() -> Self {
        Self {
            state: State::Uninitialized,
        }
    }

    pub(super) fn collect(
        &mut self,
        mut notes: Option<&mut Vec<String>>,
    ) -> Vec<TemperatureSnapshot> {
        if matches!(self.state, State::Uninitialized) {
            self.state = match Session::load(notes.as_deref_mut()) {
                Ok(session) => State::Available(session),
                Err(reason) => State::Unavailable(reason),
            };
        }

        match &self.state {
            State::Uninitialized => unreachable!(),
            State::Unavailable(reason) => {
                probe_note(&mut notes, || format!("NVML unavailable: {reason}"));
                Vec::new()
            }
            State::Available(session) => session.collect(notes),
        }
    }
}

impl Session {
    fn load(mut notes: Option<&mut Vec<String>>) -> Result<Self, String> {
        let library = open_library()?;

        let result = (|| {
            let init: NvmlInit = load_function(library, b"nvmlInit_v2\0")?;
            let shutdown: NvmlShutdown = load_function(library, b"nvmlShutdown\0")?;
            let get_count: NvmlDeviceGetCount = load_function(library, b"nvmlDeviceGetCount_v2\0")?;
            let get_handle: NvmlDeviceGetHandleByIndex =
                load_function(library, b"nvmlDeviceGetHandleByIndex_v2\0")?;
            let get_uuid: NvmlDeviceGetUuid = load_function(library, b"nvmlDeviceGetUUID\0")?;
            let get_name: NvmlDeviceGetName = load_function(library, b"nvmlDeviceGetName\0")?;
            let get_temperature: NvmlDeviceGetTemperature =
                load_function(library, b"nvmlDeviceGetTemperature\0")?;

            let init_status = unsafe { init() };
            if init_status != NVML_SUCCESS {
                return Err(format!("nvmlInit_v2 failed with status {init_status}"));
            }

            let mut count = 0_u32;
            let count_status = unsafe { get_count(&mut count) };
            if count_status != NVML_SUCCESS {
                unsafe {
                    shutdown();
                }
                return Err(format!(
                    "nvmlDeviceGetCount_v2 failed with status {count_status}"
                ));
            }

            let mut devices = Vec::with_capacity(count as usize);
            for index in 0..count {
                let mut handle = std::ptr::null_mut();
                let handle_status = unsafe { get_handle(index, &mut handle) };
                if handle_status != NVML_SUCCESS || handle.is_null() {
                    report_issue(&mut notes, || {
                        format!(
                            "NVML device {index} skipped: handle query failed with status {handle_status}"
                        )
                    });
                    continue;
                }

                let Some(uuid) = read_uuid(get_uuid, handle) else {
                    report_issue(&mut notes, || {
                        format!("NVML device {index} skipped: UUID unavailable")
                    });
                    continue;
                };
                let name = read_name(get_name, handle).unwrap_or_else(|| {
                    report_issue(&mut notes, || {
                        format!("NVML device {uuid}: name unavailable, using UUID")
                    });
                    uuid.clone()
                });

                devices.push(Device {
                    handle: handle as usize,
                    uuid,
                    name,
                });
            }

            Ok(Self {
                library,
                shutdown,
                get_temperature,
                devices,
            })
        })();

        if result.is_err() {
            close_library(library);
        }
        result
    }

    fn collect(&self, mut notes: Option<&mut Vec<String>>) -> Vec<TemperatureSnapshot> {
        let mut values = Vec::with_capacity(self.devices.len());
        for device in &self.devices {
            let mut temperature = 0_u32;
            let status = unsafe {
                (self.get_temperature)(
                    device.handle as *mut c_void,
                    NVML_TEMPERATURE_GPU,
                    &mut temperature,
                )
            };
            if status != NVML_SUCCESS {
                probe_note(&mut notes, || {
                    format!(
                        "NVML GPU {} skipped: temperature query failed with status {status}",
                        device.uuid
                    )
                });
                continue;
            }

            values.push(TemperatureSnapshot {
                id: format!("nvml:{}", device.uuid),
                name: device.name.clone(),
                celsius: f64::from(temperature),
            });
        }
        values
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        unsafe {
            (self.shutdown)();
        }
        close_library(self.library);
    }
}

fn read_uuid(get_uuid: NvmlDeviceGetUuid, handle: *mut c_void) -> Option<String> {
    let mut buffer = [0_i8; NVML_DEVICE_UUID_BUFFER_SIZE];
    let status = unsafe {
        get_uuid(
            handle,
            buffer.as_mut_ptr(),
            NVML_DEVICE_UUID_BUFFER_SIZE as c_uint,
        )
    };
    if status != NVML_SUCCESS {
        return None;
    }

    let uuid = unsafe { CStr::from_ptr(buffer.as_ptr()) };
    uuid.to_str()
        .ok()
        .filter(|uuid| !uuid.is_empty())
        .map(str::to_owned)
}

fn read_name(get_name: NvmlDeviceGetName, handle: *mut c_void) -> Option<String> {
    let mut buffer = [0_i8; NVML_DEVICE_NAME_BUFFER_SIZE];
    let status = unsafe {
        get_name(
            handle,
            buffer.as_mut_ptr(),
            NVML_DEVICE_NAME_BUFFER_SIZE as c_uint,
        )
    };
    if status != NVML_SUCCESS {
        return None;
    }

    let name = unsafe { CStr::from_ptr(buffer.as_ptr()) };
    name.to_str()
        .ok()
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
}

fn open_library() -> Result<usize, String> {
    let handle = unsafe { libc::dlopen(c"libnvidia-ml.so.1".as_ptr(), RTLD_NOW | RTLD_LOCAL) };
    if handle.is_null() {
        return Err(last_dl_error());
    }
    Ok(handle as usize)
}

fn close_library(handle: usize) {
    if handle != 0 {
        unsafe {
            libc::dlclose(handle as *mut c_void);
        }
    }
}

fn load_function<T: Copy>(handle: usize, name: &'static [u8]) -> Result<T, String> {
    debug_assert_eq!(name.last(), Some(&0));
    unsafe {
        libc::dlerror();
    }
    let symbol = unsafe { libc::dlsym(handle as *mut c_void, name.as_ptr().cast()) };
    if symbol.is_null() {
        return Err(last_dl_error());
    }
    debug_assert_eq!(mem::size_of::<T>(), mem::size_of::<*mut c_void>());
    Ok(unsafe { mem::transmute_copy::<*mut c_void, T>(&symbol) })
}

fn last_dl_error() -> String {
    let error = unsafe { libc::dlerror() };
    if error.is_null() {
        return "dynamic loader returned an unknown error".to_owned();
    }
    unsafe { CStr::from_ptr(error) }
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nvml_identity_is_namespaced() {
        let device = Device {
            handle: 1,
            uuid: "GPU-test".to_owned(),
            name: "NVIDIA Test Device".to_owned(),
        };
        assert_eq!(format!("nvml:{}", device.uuid), "nvml:GPU-test");
        assert_eq!(device.name, "NVIDIA Test Device");
    }
}
