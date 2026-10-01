use std::io;
use std::mem::size_of;
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::IO::DeviceIoControl;
use windows_sys::Win32::System::Power::{
    BATTERY_QUERY_INFORMATION, BatteryTemperature, BatteryUniqueID,
    IOCTL_BATTERY_QUERY_INFORMATION, IOCTL_BATTERY_QUERY_TAG,
};

use cclover_core::model::{Collection, TemperatureId, TemperatureSnapshot};

use super::super::diagnostics::report_issue;
use super::device::{
    DeviceRead, classify_device_read, kelvin_tenths_to_celsius, open_interface, size_of_val_u16,
    utf16_z,
};

pub(super) fn collect_battery_temperatures(
    paths: &[String],
    mut notes: Option<&mut Vec<String>>,
) -> Collection<Vec<TemperatureSnapshot>> {
    let reads = paths
        .iter()
        .map(|path| {
            (
                path.clone(),
                classify_device_read(read_battery_temperature(path)),
            )
        })
        .collect::<Vec<_>>();
    for (path, read) in &reads {
        if let DeviceRead::Failed(error) = read {
            report_issue(&mut notes, || {
                format!("battery temperature read failed for {path}: {error}")
            });
        }
    }
    project_battery_temperatures(&reads)
}

pub(super) fn project_battery_temperatures(
    reads: &[(String, DeviceRead<(String, f64)>)],
) -> Collection<Vec<TemperatureSnapshot>> {
    let total = reads.len();
    let mut values = Vec::new();
    let mut degraded = false;
    for (index, (_, read)) in reads.iter().enumerate() {
        match read {
            DeviceRead::Value((identity, celsius)) => values.push(TemperatureSnapshot {
                id: TemperatureId::from_opaque_key(format!(
                    "windows:battery:{identity}:temperature"
                )),
                name: if total == 1 {
                    "Battery".to_owned()
                } else {
                    format!("Battery {}", index + 1)
                },
                celsius: *celsius,
            }),
            DeviceRead::Empty | DeviceRead::Unsupported => {}
            DeviceRead::Failed(_) => degraded = true,
        }
    }
    if degraded {
        Collection::degraded(values)
    } else {
        Collection::available(values)
    }
}

pub(super) fn read_battery_temperature(path: &str) -> io::Result<Option<(String, f64)>> {
    let handle = open_interface(path)?;
    let result = (|| {
        let tag = battery_tag(handle)?;
        if tag == 0 {
            return Ok(None);
        }
        let mut query = BATTERY_QUERY_INFORMATION {
            BatteryTag: tag,
            InformationLevel: BatteryTemperature,
            AtRate: 0,
        };
        let mut temperature = 0_u32;
        let mut returned = 0_u32;
        // SAFETY: query/output buffers are valid and handle is live.
        if unsafe {
            DeviceIoControl(
                handle,
                IOCTL_BATTERY_QUERY_INFORMATION,
                (&mut query as *mut BATTERY_QUERY_INFORMATION).cast(),
                size_of::<BATTERY_QUERY_INFORMATION>() as u32,
                (&mut temperature as *mut u32).cast(),
                size_of::<u32>() as u32,
                &mut returned,
                null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let celsius = kelvin_tenths_to_celsius(temperature)?;
        let identity = battery_unique_id(handle, tag)?.unwrap_or_else(|| path.to_ascii_lowercase());
        Ok(Some((identity, celsius)))
    })();
    // SAFETY: handle was opened successfully and is closed exactly once here.
    unsafe { CloseHandle(handle) };
    result
}

pub(super) fn battery_tag(handle: HANDLE) -> io::Result<u32> {
    let timeout = 0_u32;
    let mut tag = 0_u32;
    let mut returned = 0_u32;
    // SAFETY: input/output buffers are valid and handle is live.
    if unsafe {
        DeviceIoControl(
            handle,
            IOCTL_BATTERY_QUERY_TAG,
            (&timeout as *const u32).cast(),
            size_of::<u32>() as u32,
            (&mut tag as *mut u32).cast(),
            size_of::<u32>() as u32,
            &mut returned,
            null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(tag)
}

pub(super) fn battery_unique_id(handle: HANDLE, tag: u32) -> io::Result<Option<String>> {
    let mut query = BATTERY_QUERY_INFORMATION {
        BatteryTag: tag,
        InformationLevel: BatteryUniqueID,
        AtRate: 0,
    };
    let mut buffer = [0_u16; 256];
    let mut returned = 0_u32;
    // SAFETY: query/output buffers are valid and handle is live.
    if unsafe {
        DeviceIoControl(
            handle,
            IOCTL_BATTERY_QUERY_INFORMATION,
            (&mut query as *mut BATTERY_QUERY_INFORMATION).cast(),
            size_of::<BATTERY_QUERY_INFORMATION>() as u32,
            buffer.as_mut_ptr().cast(),
            size_of_val_u16(&buffer) as u32,
            &mut returned,
            null_mut(),
        )
    } == 0
    {
        return Ok(None);
    }
    let used = (returned as usize / size_of::<u16>()).min(buffer.len());
    let text = utf16_z(&buffer[..used]);
    Ok((!text.is_empty()).then_some(text))
}
