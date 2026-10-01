use std::io;
use std::mem::{offset_of, size_of};
use std::ptr::{null, null_mut};

use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, HDEVINFO, SP_DEVICE_INTERFACE_DATA,
    SP_DEVICE_INTERFACE_DETAIL_DATA_W, SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces,
    SetupDiGetClassDevsW, SetupDiGetDeviceInterfaceDetailW,
};
use windows_sys::Win32::Foundation::{
    ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_ITEMS, GENERIC_READ, GetLastError, HANDLE,
    INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::core::GUID;

pub(super) const ERROR_INVALID_FUNCTION: i32 = 1;
pub(super) const ERROR_NOT_SUPPORTED: i32 = 50;
const ERROR_INVALID_PARAMETER: i32 = 87;

pub(super) enum DeviceRead<T> {
    Value(T),
    Empty,
    Unsupported,
    Failed(io::Error),
}
pub(super) fn open_interface(path: &str) -> io::Result<HANDLE> {
    let wide = wide_z(path);
    // SAFETY: wide is nul-terminated and all other arguments are valid constants/nulls.
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            null(),
            OPEN_EXISTING,
            0,
            null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        Err(io::Error::last_os_error())
    } else {
        Ok(handle)
    }
}

pub(super) struct DeviceInfoSet(HDEVINFO);

impl Drop for DeviceInfoSet {
    fn drop(&mut self) {
        // SAFETY: self.0 is a successful SetupDiGetClassDevsW result and is destroyed once.
        unsafe { SetupDiDestroyDeviceInfoList(self.0) };
    }
}

pub(super) fn device_interface_paths(class: &GUID) -> io::Result<Vec<String>> {
    // SAFETY: class is a valid GUID pointer; remaining optional pointers are null.
    let set = unsafe {
        SetupDiGetClassDevsW(
            class,
            null(),
            null_mut(),
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        )
    };
    if set == -1_isize {
        return Err(io::Error::last_os_error());
    }
    let set = DeviceInfoSet(set);
    let mut paths = Vec::new();
    for index in 0_u32.. {
        let mut interface = SP_DEVICE_INTERFACE_DATA {
            cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
            ..Default::default()
        };
        // SAFETY: set/class are live and interface is correctly initialized writable storage.
        if unsafe { SetupDiEnumDeviceInterfaces(set.0, null(), class, index, &mut interface) } == 0
        {
            // SAFETY: immediately reads the thread-local error for the failed call above.
            let error = unsafe { GetLastError() };
            if error == ERROR_NO_MORE_ITEMS {
                break;
            }
            return Err(io::Error::from_raw_os_error(error as i32));
        }

        let mut required = 0_u32;
        // SAFETY: null detail buffer with zero size is the documented size-query pattern.
        let _ = unsafe {
            SetupDiGetDeviceInterfaceDetailW(
                set.0,
                &interface,
                null_mut(),
                0,
                &mut required,
                null_mut(),
            )
        };
        // SAFETY: immediately reads the thread-local error for the size-query call above.
        let error = unsafe { GetLastError() };
        if error != ERROR_INSUFFICIENT_BUFFER || required == 0 {
            return Err(io::Error::from_raw_os_error(error as i32));
        }

        let words = (required as usize).div_ceil(size_of::<usize>());
        let mut detail_storage = vec![0_usize; words];
        let detail = detail_storage
            .as_mut_ptr()
            .cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
        // SAFETY: detail is aligned and points into a buffer at least `required` bytes long.
        unsafe { (*detail).cbSize = size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32 };
        // SAFETY: all pointers are valid for the documented sizes and set/interface are live.
        if unsafe {
            SetupDiGetDeviceInterfaceDetailW(
                set.0,
                &interface,
                detail,
                required,
                null_mut(),
                null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let path_offset = offset_of!(SP_DEVICE_INTERFACE_DETAIL_DATA_W, DevicePath);
        let max_units = (required as usize).saturating_sub(path_offset) / size_of::<u16>();
        // SAFETY: detail points to required bytes. Use a byte offset instead of taking a
        // reference to DevicePath because the x86 SDK representation is packed(1).
        let path_ptr = unsafe { detail.cast::<u8>().add(path_offset).cast::<u16>() };
        // SAFETY: path_ptr spans the remaining UTF-16 units returned by SetupAPI.
        let wide = unsafe { std::slice::from_raw_parts(path_ptr, max_units) };
        paths.push(utf16_z(wide));
    }
    Ok(paths)
}

pub(super) fn kelvin_tenths_to_celsius(value: u32) -> io::Result<f64> {
    let celsius = f64::from(value) / 10.0 - 273.15;
    if valid_celsius(celsius) {
        Ok(celsius)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("implausible temperature value {value} (0.1 K)"),
        ))
    }
}

pub(super) fn valid_celsius(value: f64) -> bool {
    (-100.0..=200.0).contains(&value)
}

pub(super) fn unsupported_ioctl(error: &io::Error) -> bool {
    matches!(
        error.raw_os_error(),
        Some(ERROR_INVALID_FUNCTION | ERROR_NOT_SUPPORTED | ERROR_INVALID_PARAMETER)
    )
}

pub(super) fn classify_device_read<T>(result: io::Result<Option<T>>) -> DeviceRead<T> {
    match result {
        Ok(Some(value)) => DeviceRead::Value(value),
        Ok(None) => DeviceRead::Empty,
        Err(error) if unsupported_ioctl(&error) => DeviceRead::Unsupported,
        Err(error) => DeviceRead::Failed(error),
    }
}

pub(super) fn wide_z(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

pub(super) fn utf16_z(value: &[u16]) -> String {
    let len = value
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(value.len());
    String::from_utf16_lossy(&value[..len]).trim().to_owned()
}

pub(super) fn size_of_val_u16<const N: usize>(_: &[u16; N]) -> usize {
    N * size_of::<u16>()
}
