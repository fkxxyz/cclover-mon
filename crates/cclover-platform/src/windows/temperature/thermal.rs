use std::io;
use std::mem::size_of;
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::System::IO::DeviceIoControl;
use windows_sys::Win32::System::Power::{IOCTL_THERMAL_READ_TEMPERATURE, THERMAL_WAIT_READ};

use cclover_core::model::{Collection, TemperatureId, TemperatureSnapshot};

use super::super::diagnostics::report_issue;
use super::device::{DeviceRead, classify_device_read, kelvin_tenths_to_celsius, open_interface};

pub(super) fn collect_thermal_zones(
    paths: &[String],
    mut notes: Option<&mut Vec<String>>,
) -> Collection<Vec<TemperatureSnapshot>> {
    let reads = paths
        .iter()
        .map(|path| (path.clone(), classify_device_read(read_thermal_zone(path))))
        .collect::<Vec<_>>();
    for (path, read) in &reads {
        if let DeviceRead::Failed(error) = read {
            report_issue(&mut notes, || {
                format!("ACPI thermal-zone read failed for {path}: {error}")
            });
        }
    }
    project_thermal_zones(&reads)
}

pub(super) fn project_thermal_zones(
    reads: &[(String, DeviceRead<f64>)],
) -> Collection<Vec<TemperatureSnapshot>> {
    let total = reads.len();
    let mut values = Vec::new();
    let mut degraded = false;
    for (index, (path, read)) in reads.iter().enumerate() {
        match read {
            DeviceRead::Value(celsius) => values.push(TemperatureSnapshot {
                id: TemperatureId::from_opaque_key(format!(
                    "windows:acpi-thermal-zone:{}",
                    path.to_ascii_lowercase()
                )),
                name: if total == 1 {
                    "ACPI Thermal Zone".to_owned()
                } else {
                    format!("ACPI Thermal Zone {}", index + 1)
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

pub(super) fn read_thermal_zone(path: &str) -> io::Result<Option<f64>> {
    let handle = open_interface(path)?;
    let input = THERMAL_WAIT_READ {
        Timeout: 0,
        LowTemperature: 0,
        HighTemperature: u32::MAX,
    };
    let mut output = 0_u32;
    let mut returned = 0_u32;
    // SAFETY: input/output buffers are valid and handle is live for this synchronous request.
    let ok = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_THERMAL_READ_TEMPERATURE,
            (&input as *const THERMAL_WAIT_READ).cast(),
            size_of::<THERMAL_WAIT_READ>() as u32,
            (&mut output as *mut u32).cast(),
            size_of::<u32>() as u32,
            &mut returned,
            null_mut(),
        )
    };
    // SAFETY: handle was opened successfully and is closed exactly once here.
    unsafe { CloseHandle(handle) };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    if output == 0 {
        return Ok(None);
    }
    kelvin_tenths_to_celsius(output).map(Some)
}
