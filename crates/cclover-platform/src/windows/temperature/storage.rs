use std::io;
use std::mem::{offset_of, size_of};
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{
    CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::IO::DeviceIoControl;
use windows_sys::Win32::System::Ioctl::{
    IDEREGS, IOCTL_STORAGE_QUERY_PROPERTY, PropertyStandardQuery, READ_ATTRIBUTE_BUFFER_SIZE,
    READ_ATTRIBUTES, SENDCMDINPARAMS, SENDCMDOUTPARAMS, SMART_CMD, SMART_CYL_HI, SMART_CYL_LOW,
    SMART_RCV_DRIVE_DATA, STORAGE_PROPERTY_QUERY, STORAGE_TEMPERATURE_DATA_DESCRIPTOR,
    STORAGE_TEMPERATURE_INFO, StorageDeviceTemperatureProperty,
};

use cclover_core::model::{Collection, TemperatureId, TemperatureSnapshot};

use super::super::diagnostics::report_issue;
use super::super::native;
use super::device::{unsupported_ioctl, valid_celsius, wide_z};

const MAX_PHYSICAL_DRIVES: u32 = 64;
const ERROR_FILE_NOT_FOUND: i32 = 2;
const ERROR_PATH_NOT_FOUND: i32 = 3;
pub(super) const ATA_SMART_TEMPERATURE: u8 = 194;
pub(super) const ATA_SMART_AIRFLOW_TEMPERATURE: u8 = 190;
const ATA_SMART_ATTRIBUTE_COUNT: usize = 30;
pub(super) const ATA_SMART_ATTRIBUTE_SIZE: usize = 12;
pub(super) const ATA_SMART_ATTRIBUTE_OFFSET: usize = 2;

#[derive(Clone, Copy)]
pub(super) enum StorageTemperatureProjection {
    Product,
    Diagnostic,
}

pub(super) struct StorageDevice {
    pub(super) disk_number: u32,
    pub(super) identity: String,
    pub(super) fallback_identity: bool,
}

pub(super) enum StructuredTemperatureRead {
    Sensors(Vec<StorageTemperature>),
    Unsupported,
    Failed(io::Error),
}

pub(super) enum SmartTemperatureRead {
    Temperature(f64),
    Empty,
    Unsupported,
    Failed(io::Error),
}
pub(super) fn discover_storage_devices(issues: &mut Vec<String>) -> Vec<StorageDevice> {
    let mut devices = Vec::new();
    for disk_number in 0..MAX_PHYSICAL_DRIVES {
        let path = format!(r"\\.\PhysicalDrive{disk_number}");
        let wide = wide_z(&path);
        // SAFETY: wide is nul-terminated; metadata queries need no data access rights.
        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                null(),
                OPEN_EXISTING,
                0,
                null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            let error = io::Error::last_os_error();
            if !matches!(
                error.raw_os_error(),
                Some(ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND)
            ) {
                issues.push(format!(
                    "storage temperature discovery failed for {path}: {error}"
                ));
            }
            continue;
        }
        let (identity, fallback_identity) = native::storage_descriptor(handle)
            .unwrap_or_else(|| (format!("physical-drive:{disk_number}"), true));
        // SAFETY: handle was opened successfully and is closed exactly once here.
        unsafe { CloseHandle(handle) };
        devices.push(StorageDevice {
            disk_number,
            identity,
            fallback_identity,
        });
    }
    devices
}

pub(super) fn collect_storage_temperatures(
    devices: &[StorageDevice],
    projection: StorageTemperatureProjection,
    mut notes: Option<&mut Vec<String>>,
) -> Collection<Vec<TemperatureSnapshot>> {
    let mut values = Vec::new();
    let mut degraded = false;

    for device in devices {
        let disk_number = device.disk_number;
        let path = format!(r"\\.\PhysicalDrive{disk_number}");
        let wide = wide_z(&path);
        // SAFETY: wide is nul-terminated; metadata IOCTLs need no data access rights.
        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                null(),
                OPEN_EXISTING,
                0,
                null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            let error = io::Error::last_os_error();
            if !matches!(
                error.raw_os_error(),
                Some(ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND)
            ) {
                degraded = true;
                report_issue(&mut notes, || {
                    format!("storage temperature open failed for {path}: {error}")
                });
            }
            continue;
        }

        let structured = classify_structured_temperature(storage_temperature_info(handle));
        let smart = if storage_needs_smart(&structured) {
            Some(classify_smart_temperature(ata_smart_temperature(&path)))
        } else {
            None
        };
        report_storage_temperature_issues(&path, &structured, smart.as_ref(), &mut notes);
        let (mut projected, device_degraded) =
            project_storage_temperature(device, projection, &structured, smart.as_ref());
        values.append(&mut projected);
        degraded |= device_degraded;

        // SAFETY: handle was opened successfully and is closed exactly once here.
        unsafe { CloseHandle(handle) };
    }

    if degraded {
        Collection::degraded(values)
    } else {
        Collection::available(values)
    }
}

pub(super) fn classify_structured_temperature(
    result: io::Result<Vec<StorageTemperature>>,
) -> StructuredTemperatureRead {
    match result {
        Ok(sensors) => StructuredTemperatureRead::Sensors(sensors),
        Err(error) if unsupported_ioctl(&error) => StructuredTemperatureRead::Unsupported,
        Err(error) => StructuredTemperatureRead::Failed(error),
    }
}

pub(super) fn classify_smart_temperature(result: io::Result<Option<f64>>) -> SmartTemperatureRead {
    match result {
        Ok(Some(celsius)) => SmartTemperatureRead::Temperature(celsius),
        Ok(None) => SmartTemperatureRead::Empty,
        Err(error) if unsupported_ioctl(&error) => SmartTemperatureRead::Unsupported,
        Err(error) => SmartTemperatureRead::Failed(error),
    }
}

pub(super) fn storage_needs_smart(structured: &StructuredTemperatureRead) -> bool {
    !matches!(structured, StructuredTemperatureRead::Sensors(sensors) if !sensors.is_empty())
}

pub(super) fn storage_fallback_expected(structured: &StructuredTemperatureRead) -> bool {
    match structured {
        StructuredTemperatureRead::Sensors(sensors) => sensors.is_empty(),
        StructuredTemperatureRead::Unsupported => true,
        StructuredTemperatureRead::Failed(_) => false,
    }
}

pub(super) fn project_storage_temperature(
    device: &StorageDevice,
    projection: StorageTemperatureProjection,
    structured: &StructuredTemperatureRead,
    smart: Option<&SmartTemperatureRead>,
) -> (Vec<TemperatureSnapshot>, bool) {
    if let StructuredTemperatureRead::Sensors(sensors) = structured {
        if !sensors.is_empty() {
            let values = match projection {
                StorageTemperatureProjection::Product => primary_storage_temperature(sensors)
                    .map(|sensor| vec![storage_temperature_snapshot(device, *sensor, false)])
                    .unwrap_or_default(),
                StorageTemperatureProjection::Diagnostic => sensors
                    .iter()
                    .copied()
                    .map(|sensor| storage_temperature_snapshot(device, sensor, true))
                    .collect(),
            };
            return (values, device.fallback_identity);
        }
    }

    if let Some(SmartTemperatureRead::Temperature(celsius)) = smart {
        return (
            vec![TemperatureSnapshot {
                id: TemperatureId::from_opaque_key(format!(
                    "windows:storage:{}:temperature:0",
                    device.identity
                )),
                name: format!("Disk {}", device.disk_number),
                celsius: *celsius,
            }],
            device.fallback_identity,
        );
    }

    (Vec::new(), !storage_fallback_expected(structured))
}

pub(super) fn report_storage_temperature_issues(
    path: &str,
    structured: &StructuredTemperatureRead,
    smart: Option<&SmartTemperatureRead>,
    notes: &mut Option<&mut Vec<String>>,
) {
    let fallback_expected = storage_fallback_expected(structured);
    match (structured, smart) {
        (StructuredTemperatureRead::Failed(error), Some(SmartTemperatureRead::Empty)) => {
            report_issue(notes, || {
                format!("storage temperature query failed for {path}: {error}")
            });
        }
        (StructuredTemperatureRead::Failed(error), Some(SmartTemperatureRead::Unsupported)) => {
            report_issue(notes, || {
                format!(
                    "storage temperature query failed for {path}: {error}; ATA SMART fallback unsupported"
                )
            });
        }
        (
            StructuredTemperatureRead::Failed(error),
            Some(SmartTemperatureRead::Failed(smart_error)),
        ) => {
            report_issue(notes, || {
                format!(
                    "storage temperature query failed for {path}: {error}; ATA SMART fallback failed: {smart_error}"
                )
            });
        }
        (_, Some(SmartTemperatureRead::Failed(smart_error))) if fallback_expected => {
            report_issue(notes, || {
                format!("ATA SMART temperature fallback unavailable for {path}: {smart_error}")
            });
        }
        _ => {}
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct StorageTemperature {
    pub(super) index: u16,
    pub(super) celsius: f64,
}

pub(super) fn primary_storage_temperature(
    sensors: &[StorageTemperature],
) -> Option<&StorageTemperature> {
    sensors
        .iter()
        .find(|sensor| sensor.index == 0)
        .or_else(|| sensors.iter().min_by_key(|sensor| sensor.index))
}

pub(super) fn storage_temperature_snapshot(
    device: &StorageDevice,
    sensor: StorageTemperature,
    diagnostic: bool,
) -> TemperatureSnapshot {
    let disk_number = device.disk_number;
    let name = if diagnostic && sensor.index != 0 {
        format!("Disk {disk_number} Sensor {}", sensor.index)
    } else {
        format!("Disk {disk_number}")
    };
    TemperatureSnapshot {
        id: TemperatureId::from_opaque_key(format!(
            "windows:storage:{}:temperature:{}",
            device.identity, sensor.index
        )),
        name,
        celsius: sensor.celsius,
    }
}

pub(super) fn storage_temperature_info(handle: HANDLE) -> io::Result<Vec<StorageTemperature>> {
    let query = STORAGE_PROPERTY_QUERY {
        PropertyId: StorageDeviceTemperatureProperty,
        QueryType: PropertyStandardQuery,
        AdditionalParameters: [0],
    };
    let mut storage = vec![0_usize; 4096_usize.div_ceil(size_of::<usize>())];
    let output_bytes = storage.len() * size_of::<usize>();
    let mut returned = 0_u32;
    // SAFETY: query and aligned output storage are valid for the stated sizes; handle is live.
    if unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            (&query as *const STORAGE_PROPERTY_QUERY).cast(),
            size_of::<STORAGE_PROPERTY_QUERY>() as u32,
            storage.as_mut_ptr().cast(),
            output_bytes as u32,
            &mut returned,
            null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }

    let base = offset_of!(STORAGE_TEMPERATURE_DATA_DESCRIPTOR, TemperatureInfo);
    if (returned as usize) < base {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "truncated storage temperature descriptor",
        ));
    }
    // SAFETY: storage is aligned to usize and returned contains the fixed descriptor prefix.
    let descriptor = unsafe {
        &*(storage
            .as_ptr()
            .cast::<STORAGE_TEMPERATURE_DATA_DESCRIPTOR>())
    };
    let count = descriptor.InfoCount as usize;
    let required = base.saturating_add(count.saturating_mul(size_of::<STORAGE_TEMPERATURE_INFO>()));
    let available = (returned as usize).min(descriptor.Size as usize);
    if required > available || required > output_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "truncated storage temperature sensor array",
        ));
    }
    // SAFETY: required byte count for the variable-length array was validated above.
    let sensors = unsafe { std::slice::from_raw_parts(descriptor.TemperatureInfo.as_ptr(), count) };
    Ok(sensors
        .iter()
        .filter_map(|sensor| {
            let celsius = f64::from(sensor.Temperature);
            valid_celsius(celsius).then_some(StorageTemperature {
                index: sensor.Index,
                celsius,
            })
        })
        .collect())
}

pub(super) fn ata_smart_temperature(path: &str) -> io::Result<Option<f64>> {
    let wide = wide_z(path);
    // SMART_RCV_DRIVE_DATA requires FILE_READ_ACCESS | FILE_WRITE_ACCESS. Keep this
    // fallback separate from the zero-access structured temperature query.
    // SAFETY: wide is nul-terminated and all other arguments are valid constants/nulls.
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            null(),
            OPEN_EXISTING,
            0,
            null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }

    let input = SENDCMDINPARAMS {
        cBufferSize: READ_ATTRIBUTE_BUFFER_SIZE,
        irDriveRegs: IDEREGS {
            bFeaturesReg: READ_ATTRIBUTES as u8,
            bSectorCountReg: 1,
            bSectorNumberReg: 1,
            bCylLowReg: SMART_CYL_LOW as u8,
            bCylHighReg: SMART_CYL_HI as u8,
            bDriveHeadReg: 0xA0,
            bCommandReg: SMART_CMD as u8,
            bReserved: 0,
        },
        ..Default::default()
    };
    let output_prefix = offset_of!(SENDCMDOUTPARAMS, bBuffer);
    let mut output = vec![0_u8; output_prefix + READ_ATTRIBUTE_BUFFER_SIZE as usize];
    let mut returned = 0_u32;
    // SAFETY: input/output buffers satisfy SMART_RCV_DRIVE_DATA's documented sizes and
    // handle has the read/write access required by the IOCTL.
    let ok = unsafe {
        DeviceIoControl(
            handle,
            SMART_RCV_DRIVE_DATA,
            (&input as *const SENDCMDINPARAMS).cast(),
            offset_of!(SENDCMDINPARAMS, bBuffer) as u32,
            output.as_mut_ptr().cast(),
            output.len() as u32,
            &mut returned,
            null_mut(),
        )
    };
    // SAFETY: handle was opened successfully and is closed exactly once here.
    unsafe { CloseHandle(handle) };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    if (returned as usize) < output.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "truncated ATA SMART attribute response",
        ));
    }
    Ok(ata_smart_temperature_from_page(&output[output_prefix..]))
}

pub(super) fn ata_smart_temperature_from_page(page: &[u8]) -> Option<f64> {
    let required =
        ATA_SMART_ATTRIBUTE_OFFSET + ATA_SMART_ATTRIBUTE_COUNT * ATA_SMART_ATTRIBUTE_SIZE;
    if page.len() < required {
        return None;
    }

    [ATA_SMART_TEMPERATURE, ATA_SMART_AIRFLOW_TEMPERATURE]
        .into_iter()
        .find_map(|wanted| {
            (0..ATA_SMART_ATTRIBUTE_COUNT).find_map(|index| {
                let offset = ATA_SMART_ATTRIBUTE_OFFSET + index * ATA_SMART_ATTRIBUTE_SIZE;
                let entry = &page[offset..offset + ATA_SMART_ATTRIBUTE_SIZE];
                if entry[0] != wanted {
                    return None;
                }
                let celsius = f64::from(entry[5]);
                (1.0..=125.0).contains(&celsius).then_some(celsius)
            })
        })
}
