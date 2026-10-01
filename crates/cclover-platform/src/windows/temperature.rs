#![allow(unsafe_code)]

use std::io;
use std::mem::{offset_of, size_of};
use std::ptr::{null, null_mut};

use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, HDEVINFO, SP_DEVICE_INTERFACE_DATA,
    SP_DEVICE_INTERFACE_DETAIL_DATA_W, SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces,
    SetupDiGetClassDevsW, SetupDiGetDeviceInterfaceDetailW,
};
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_ITEMS, GENERIC_READ, GENERIC_WRITE,
    GetLastError, HANDLE, INVALID_HANDLE_VALUE,
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
use windows_sys::Win32::System::Power::{
    BATTERY_QUERY_INFORMATION, BatteryTemperature, BatteryUniqueID, GUID_DEVICE_BATTERY,
    GUID_DEVICE_THERMAL_ZONE, IOCTL_BATTERY_QUERY_INFORMATION, IOCTL_BATTERY_QUERY_TAG,
    IOCTL_THERMAL_READ_TEMPERATURE, THERMAL_WAIT_READ,
};
use windows_sys::core::GUID;

use cclover_core::model::{Collection, TemperatureId, TemperatureSnapshot};

use super::diagnostics::report_issue;
use super::hardware;
use super::native;

const MAX_PHYSICAL_DRIVES: u32 = 64;
const ERROR_FILE_NOT_FOUND: i32 = 2;
const ERROR_PATH_NOT_FOUND: i32 = 3;
const ERROR_INVALID_FUNCTION: i32 = 1;
const ERROR_NOT_SUPPORTED: i32 = 50;
const ERROR_INVALID_PARAMETER: i32 = 87;
const ATA_SMART_TEMPERATURE: u8 = 194;
const ATA_SMART_AIRFLOW_TEMPERATURE: u8 = 190;
const ATA_SMART_ATTRIBUTE_COUNT: usize = 30;
const ATA_SMART_ATTRIBUTE_SIZE: usize = 12;
const ATA_SMART_ATTRIBUTE_OFFSET: usize = 2;

pub(super) struct Collector {
    storage: Vec<StorageDevice>,
    thermal_paths: Vec<String>,
    battery_paths: Vec<String>,
    discovery_issues: Vec<String>,
}

#[derive(Clone, Copy)]
enum StorageTemperatureProjection {
    Product,
    Diagnostic,
}

struct StorageDevice {
    disk_number: u32,
    identity: String,
    fallback_identity: bool,
}

enum DeviceRead<T> {
    Value(T),
    Empty,
    Unsupported,
    Failed(io::Error),
}

enum StructuredTemperatureRead {
    Sensors(Vec<StorageTemperature>),
    Unsupported,
    Failed(io::Error),
}

enum SmartTemperatureRead {
    Temperature(f64),
    Empty,
    Unsupported,
    Failed(io::Error),
}

impl Collector {
    pub(super) fn new() -> Self {
        let mut discovery_issues = Vec::new();
        let storage = discover_storage_devices(&mut discovery_issues);
        let thermal_paths =
            device_interface_paths(&GUID_DEVICE_THERMAL_ZONE).unwrap_or_else(|error| {
                discovery_issues.push(format!("ACPI thermal-zone discovery failed: {error}"));
                Vec::new()
            });
        let battery_paths = device_interface_paths(&GUID_DEVICE_BATTERY).unwrap_or_else(|error| {
            discovery_issues.push(format!("battery discovery failed: {error}"));
            Vec::new()
        });
        Self {
            storage,
            thermal_paths,
            battery_paths,
            discovery_issues,
        }
    }

    pub(super) fn collect(
        &self,
        notes: Option<&mut Vec<String>>,
    ) -> Collection<Vec<TemperatureSnapshot>> {
        self.collect_with_storage_projection(StorageTemperatureProjection::Product, notes)
    }

    pub(super) fn collect_diagnostic(
        &self,
        notes: Option<&mut Vec<String>>,
    ) -> Collection<Vec<TemperatureSnapshot>> {
        self.collect_with_storage_projection(StorageTemperatureProjection::Diagnostic, notes)
    }

    fn collect_with_storage_projection(
        &self,
        projection: StorageTemperatureProjection,
        mut notes: Option<&mut Vec<String>>,
    ) -> Collection<Vec<TemperatureSnapshot>> {
        for issue in &self.discovery_issues {
            report_issue(&mut notes, || issue.clone());
        }
        let storage = collect_storage_temperatures(&self.storage, projection, notes.as_deref_mut());
        let thermal = collect_thermal_zones(&self.thermal_paths, notes.as_deref_mut());
        let battery = collect_battery_temperatures(&self.battery_paths, notes);
        let merged = hardware::merge_temperature_sources([storage, thermal, battery]);
        if self.discovery_issues.is_empty() {
            merged
        } else {
            match merged {
                Collection::Available(values) => Collection::degraded(values),
                other => other,
            }
        }
    }
}

impl Default for Collector {
    fn default() -> Self {
        Self::new()
    }
}

fn discover_storage_devices(issues: &mut Vec<String>) -> Vec<StorageDevice> {
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

fn collect_storage_temperatures(
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

fn classify_structured_temperature(
    result: io::Result<Vec<StorageTemperature>>,
) -> StructuredTemperatureRead {
    match result {
        Ok(sensors) => StructuredTemperatureRead::Sensors(sensors),
        Err(error) if unsupported_ioctl(&error) => StructuredTemperatureRead::Unsupported,
        Err(error) => StructuredTemperatureRead::Failed(error),
    }
}

fn classify_smart_temperature(result: io::Result<Option<f64>>) -> SmartTemperatureRead {
    match result {
        Ok(Some(celsius)) => SmartTemperatureRead::Temperature(celsius),
        Ok(None) => SmartTemperatureRead::Empty,
        Err(error) if unsupported_ioctl(&error) => SmartTemperatureRead::Unsupported,
        Err(error) => SmartTemperatureRead::Failed(error),
    }
}

fn storage_needs_smart(structured: &StructuredTemperatureRead) -> bool {
    !matches!(structured, StructuredTemperatureRead::Sensors(sensors) if !sensors.is_empty())
}

fn storage_fallback_expected(structured: &StructuredTemperatureRead) -> bool {
    match structured {
        StructuredTemperatureRead::Sensors(sensors) => sensors.is_empty(),
        StructuredTemperatureRead::Unsupported => true,
        StructuredTemperatureRead::Failed(_) => false,
    }
}

fn project_storage_temperature(
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

fn report_storage_temperature_issues(
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
struct StorageTemperature {
    index: u16,
    celsius: f64,
}

fn primary_storage_temperature(sensors: &[StorageTemperature]) -> Option<&StorageTemperature> {
    sensors
        .iter()
        .find(|sensor| sensor.index == 0)
        .or_else(|| sensors.iter().min_by_key(|sensor| sensor.index))
}

fn storage_temperature_snapshot(
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

fn storage_temperature_info(handle: HANDLE) -> io::Result<Vec<StorageTemperature>> {
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

fn ata_smart_temperature(path: &str) -> io::Result<Option<f64>> {
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

fn ata_smart_temperature_from_page(page: &[u8]) -> Option<f64> {
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

fn collect_thermal_zones(
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

fn project_thermal_zones(
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

fn read_thermal_zone(path: &str) -> io::Result<Option<f64>> {
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

fn collect_battery_temperatures(
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

fn project_battery_temperatures(
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

fn read_battery_temperature(path: &str) -> io::Result<Option<(String, f64)>> {
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

fn battery_tag(handle: HANDLE) -> io::Result<u32> {
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

fn battery_unique_id(handle: HANDLE, tag: u32) -> io::Result<Option<String>> {
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

fn open_interface(path: &str) -> io::Result<HANDLE> {
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

struct DeviceInfoSet(HDEVINFO);

impl Drop for DeviceInfoSet {
    fn drop(&mut self) {
        // SAFETY: self.0 is a successful SetupDiGetClassDevsW result and is destroyed once.
        unsafe { SetupDiDestroyDeviceInfoList(self.0) };
    }
}

fn device_interface_paths(class: &GUID) -> io::Result<Vec<String>> {
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

fn kelvin_tenths_to_celsius(value: u32) -> io::Result<f64> {
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

fn valid_celsius(value: f64) -> bool {
    (-100.0..=200.0).contains(&value)
}

fn unsupported_ioctl(error: &io::Error) -> bool {
    matches!(
        error.raw_os_error(),
        Some(ERROR_INVALID_FUNCTION | ERROR_NOT_SUPPORTED | ERROR_INVALID_PARAMETER)
    )
}

fn classify_device_read<T>(result: io::Result<Option<T>>) -> DeviceRead<T> {
    match result {
        Ok(Some(value)) => DeviceRead::Value(value),
        Ok(None) => DeviceRead::Empty,
        Err(error) if unsupported_ioctl(&error) => DeviceRead::Unsupported,
        Err(error) => DeviceRead::Failed(error),
    }
}

fn wide_z(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn utf16_z(value: &[u16]) -> String {
    let len = value
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(value.len());
    String::from_utf16_lossy(&value[..len]).trim().to_owned()
}

fn size_of_val_u16<const N: usize>(_: &[u16; N]) -> usize {
    N * size_of::<u16>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kelvin_tenths_conversion_matches_windows_units() {
        assert!((kelvin_tenths_to_celsius(3_001).unwrap() - 26.95).abs() < 1e-9);
    }

    #[test]
    fn invalid_kelvin_temperature_is_rejected() {
        assert!(kelvin_tenths_to_celsius(0).is_err());
    }

    #[test]
    fn storage_product_temperature_prefers_index_zero_even_when_unsorted() {
        let sensors = [
            StorageTemperature {
                index: 4,
                celsius: 44.0,
            },
            StorageTemperature {
                index: 0,
                celsius: 40.0,
            },
            StorageTemperature {
                index: 2,
                celsius: 42.0,
            },
        ];
        assert_eq!(primary_storage_temperature(&sensors), Some(&sensors[1]));
    }

    #[test]
    fn storage_product_temperature_falls_back_to_lowest_index() {
        let sensors = [
            StorageTemperature {
                index: 3,
                celsius: 43.0,
            },
            StorageTemperature {
                index: 1,
                celsius: 41.0,
            },
            StorageTemperature {
                index: 2,
                celsius: 42.0,
            },
        ];
        assert_eq!(primary_storage_temperature(&sensors), Some(&sensors[1]));
    }

    #[test]
    fn storage_product_temperature_handles_empty_sensor_set() {
        assert_eq!(primary_storage_temperature(&[]), None);
    }

    #[test]
    fn ata_smart_temperature_prefers_attribute_194() {
        let mut page = [0_u8; READ_ATTRIBUTE_BUFFER_SIZE as usize];
        let airflow = ATA_SMART_ATTRIBUTE_OFFSET;
        page[airflow] = ATA_SMART_AIRFLOW_TEMPERATURE;
        page[airflow + 5] = 39;
        let primary = ATA_SMART_ATTRIBUTE_OFFSET + ATA_SMART_ATTRIBUTE_SIZE;
        page[primary] = ATA_SMART_TEMPERATURE;
        page[primary + 5] = 42;
        assert_eq!(ata_smart_temperature_from_page(&page), Some(42.0));
    }

    #[test]
    fn ata_smart_temperature_falls_back_to_attribute_190() {
        let mut page = [0_u8; READ_ATTRIBUTE_BUFFER_SIZE as usize];
        page[ATA_SMART_ATTRIBUTE_OFFSET] = ATA_SMART_AIRFLOW_TEMPERATURE;
        page[ATA_SMART_ATTRIBUTE_OFFSET + 5] = 37;
        assert_eq!(ata_smart_temperature_from_page(&page), Some(37.0));
    }

    #[test]
    fn ata_smart_temperature_rejects_implausible_raw_value() {
        let mut page = [0_u8; READ_ATTRIBUTE_BUFFER_SIZE as usize];
        page[ATA_SMART_ATTRIBUTE_OFFSET] = ATA_SMART_TEMPERATURE;
        page[ATA_SMART_ATTRIBUTE_OFFSET + 5] = 200;
        assert_eq!(ata_smart_temperature_from_page(&page), None);
    }

    fn storage_device(fallback_identity: bool) -> StorageDevice {
        StorageDevice {
            disk_number: 7,
            identity: "disk-id".to_owned(),
            fallback_identity,
        }
    }

    #[test]
    fn storage_policy_selects_one_product_sensor_and_all_diagnostic_sensors() {
        let structured = StructuredTemperatureRead::Sensors(vec![
            StorageTemperature {
                index: 3,
                celsius: 43.0,
            },
            StorageTemperature {
                index: 0,
                celsius: 40.0,
            },
        ]);
        let device = storage_device(false);

        let (product, degraded) = project_storage_temperature(
            &device,
            StorageTemperatureProjection::Product,
            &structured,
            None,
        );
        assert!(!degraded);
        assert_eq!(product.len(), 1);
        assert_eq!(
            product[0].id.as_opaque_key(),
            "windows:storage:disk-id:temperature:0"
        );
        assert_eq!(product[0].celsius, 40.0);

        let (diagnostic, degraded) = project_storage_temperature(
            &device,
            StorageTemperatureProjection::Diagnostic,
            &structured,
            None,
        );
        assert!(!degraded);
        assert_eq!(diagnostic.len(), 2);
        assert_eq!(
            diagnostic[0].id.as_opaque_key(),
            "windows:storage:disk-id:temperature:3"
        );
        assert_eq!(
            diagnostic[1].id.as_opaque_key(),
            "windows:storage:disk-id:temperature:0"
        );
    }

    #[test]
    fn storage_policy_uses_smart_for_unsupported_structured_temperature() {
        let device = storage_device(false);
        let (values, degraded) = project_storage_temperature(
            &device,
            StorageTemperatureProjection::Product,
            &StructuredTemperatureRead::Unsupported,
            Some(&SmartTemperatureRead::Temperature(38.0)),
        );
        assert!(!degraded);
        assert_eq!(values.len(), 1);
        assert_eq!(
            values[0].id.as_opaque_key(),
            "windows:storage:disk-id:temperature:0"
        );
        assert_eq!(values[0].celsius, 38.0);
    }

    #[test]
    fn storage_policy_distinguishes_expected_fallback_absence_from_hard_failure() {
        let device = storage_device(false);
        let (values, degraded) = project_storage_temperature(
            &device,
            StorageTemperatureProjection::Product,
            &StructuredTemperatureRead::Sensors(Vec::new()),
            Some(&SmartTemperatureRead::Unsupported),
        );
        assert!(values.is_empty());
        assert!(!degraded);

        let (values, degraded) = project_storage_temperature(
            &device,
            StorageTemperatureProjection::Product,
            &StructuredTemperatureRead::Failed(io::Error::other("structured failure")),
            Some(&SmartTemperatureRead::Empty),
        );
        assert!(values.is_empty());
        assert!(degraded);
    }

    #[test]
    fn storage_policy_marks_locator_identity_as_degraded() {
        let device = storage_device(true);
        let (values, degraded) = project_storage_temperature(
            &device,
            StorageTemperatureProjection::Product,
            &StructuredTemperatureRead::Sensors(vec![StorageTemperature {
                index: 2,
                celsius: 42.0,
            }]),
            None,
        );
        assert_eq!(
            values[0].id.as_opaque_key(),
            "windows:storage:disk-id:temperature:2"
        );
        assert!(degraded);
    }

    #[test]
    fn storage_native_results_preserve_capability_and_hard_failure_classes() {
        assert!(matches!(
            classify_structured_temperature(Err(io::Error::from_raw_os_error(ERROR_NOT_SUPPORTED))),
            StructuredTemperatureRead::Unsupported
        ));
        assert!(matches!(
            classify_structured_temperature(Err(io::Error::other("query failed"))),
            StructuredTemperatureRead::Failed(_)
        ));
        assert!(matches!(
            classify_smart_temperature(Err(io::Error::from_raw_os_error(ERROR_INVALID_FUNCTION))),
            SmartTemperatureRead::Unsupported
        ));
        assert!(matches!(
            classify_smart_temperature(Err(io::Error::other("SMART failed"))),
            SmartTemperatureRead::Failed(_)
        ));
    }

    #[test]
    fn acpi_policy_keeps_partial_success_and_typed_identity() {
        let reads = vec![
            ("THERMAL-A".to_owned(), DeviceRead::Value(45.0)),
            ("THERMAL-B".to_owned(), DeviceRead::Unsupported),
            (
                "THERMAL-C".to_owned(),
                DeviceRead::Failed(io::Error::other("read failed")),
            ),
        ];
        let result = project_thermal_zones(&reads);
        let Collection::Degraded(values) = result else {
            panic!("hard device failure must degrade partial ACPI data");
        };
        assert_eq!(values.len(), 1);
        assert_eq!(
            values[0].id.as_opaque_key(),
            "windows:acpi-thermal-zone:thermal-a"
        );
        assert_eq!(values[0].celsius, 45.0);
    }

    #[test]
    fn acpi_unsupported_and_empty_are_successful_empty_observations() {
        let result = project_thermal_zones(&[
            ("a".to_owned(), DeviceRead::Unsupported),
            ("b".to_owned(), DeviceRead::Empty),
        ]);
        assert!(matches!(result, Collection::Available(values) if values.is_empty()));
    }

    #[test]
    fn device_read_classification_keeps_unsupported_empty_and_invalid_distinct() {
        assert!(matches!(
            classify_device_read::<f64>(Ok(None)),
            DeviceRead::Empty
        ));
        assert!(matches!(
            classify_device_read::<f64>(Err(io::Error::from_raw_os_error(ERROR_NOT_SUPPORTED))),
            DeviceRead::Unsupported
        ));
        assert!(matches!(
            classify_device_read::<f64>(Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid temperature"
            ))),
            DeviceRead::Failed(_)
        ));
    }

    #[test]
    fn battery_policy_keeps_partial_success_and_stable_identity() {
        let reads = vec![
            (
                "path-a".to_owned(),
                DeviceRead::Value(("battery-uid".to_owned(), 31.5)),
            ),
            ("path-b".to_owned(), DeviceRead::Unsupported),
            (
                "path-c".to_owned(),
                DeviceRead::Failed(io::Error::other("read failed")),
            ),
        ];
        let result = project_battery_temperatures(&reads);
        let Collection::Degraded(values) = result else {
            panic!("hard battery failure must degrade partial data");
        };
        assert_eq!(values.len(), 1);
        assert_eq!(
            values[0].id.as_opaque_key(),
            "windows:battery:battery-uid:temperature"
        );
        assert_eq!(values[0].celsius, 31.5);
    }

    #[test]
    fn battery_empty_and_unsupported_are_not_hard_failures() {
        let result = project_battery_temperatures(&[
            ("a".to_owned(), DeviceRead::Empty),
            ("b".to_owned(), DeviceRead::Unsupported),
        ]);
        assert!(matches!(result, Collection::Available(values) if values.is_empty()));

        let result = project_battery_temperatures(&[(
            "c".to_owned(),
            DeviceRead::Failed(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid battery temperature",
            )),
        )]);
        assert!(matches!(result, Collection::Degraded(values) if values.is_empty()));
    }
}
