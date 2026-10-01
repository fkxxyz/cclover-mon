#![allow(unsafe_code)]

use std::io;
use std::mem::{size_of, size_of_val, zeroed};
use std::ptr::{null, null_mut};

use windows_sys::Wdk::System::SystemInformation::{
    NtQuerySystemInformation, SystemProcessInformation,
};
use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    ConvertInterfaceLuidToAlias, ConvertInterfaceLuidToGuid, FreeMibTable, GetIfTable2,
    MIB_IF_ROW2, MIB_IF_TABLE2,
};
use windows_sys::Win32::NetworkManagement::Ndis::{IfOperStatusUp, NET_LUID_LH};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, FindFirstVolumeW, FindNextVolumeW,
    FindVolumeClose, GetDriveTypeW, GetLogicalDrives, IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS,
    OPEN_EXISTING, QueryDosDeviceW,
};
use windows_sys::Win32::System::IO::DeviceIoControl;
use windows_sys::Win32::System::Ioctl::{
    DISK_PERFORMANCE, IOCTL_DISK_PERFORMANCE, IOCTL_STORAGE_QUERY_PROPERTY, PropertyStandardQuery,
    STORAGE_DEVICE_DESCRIPTOR, STORAGE_PROPERTY_QUERY, StorageDeviceProperty, VOLUME_DISK_EXTENTS,
};
use windows_sys::Win32::System::ProcessStatus::{ENUM_PAGE_FILE_INFORMATION, K32EnumPageFilesW};
use windows_sys::Win32::System::SystemInformation::{
    GetSystemInfo, GlobalMemoryStatusEx, MEMORYSTATUSEX, SYSTEM_INFO,
};
use windows_sys::Win32::System::Threading::{
    ALL_PROCESSOR_GROUPS, GetActiveProcessorCount, GetSystemTimes,
};
use windows_sys::Win32::System::WindowsProgramming::{
    DRIVE_FIXED, DRIVE_REMOVABLE, SYSTEM_PROCESS_INFORMATION,
};
use windows_sys::core::BOOL;

const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC0000004_u32 as i32;
const ERROR_SUCCESS: u32 = 0;
const MAX_PHYSICAL_DRIVES: u32 = 64;
const HARDWARE_INTERFACE_FLAG: u8 = 1 << 0;

pub(super) struct CpuTimes {
    pub idle: u64,
    pub kernel: u64,
    pub user: u64,
    pub logical_cpu_count: usize,
}
pub(super) struct MemoryStatus {
    pub total_phys: u64,
    pub avail_phys: u64,
}
pub(super) struct PageFileStatus {
    pub total_bytes: u64,
    pub used_bytes: u64,
}
pub(super) struct NativeProcess {
    pub pid: usize,
    pub name: String,
    pub create_time: u64,
    pub user_time: u64,
    pub kernel_time: u64,
    pub working_set: usize,
}
pub(super) struct NativeNetwork {
    pub guid: String,
    pub name: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}
pub(super) struct NativeDisk {
    pub disk_number: u32,
    pub identity: String,
    pub read_bytes: u64,
    pub write_bytes: u64,
}
pub(super) struct NativeDisks {
    pub disks: Vec<NativeDisk>,
    pub fallback_identities: usize,
}
pub(super) struct DriveBinding {
    pub label: String,
    pub disk_numbers: Vec<u32>,
}

pub(super) struct DriveBindings {
    pub bindings: Vec<DriveBinding>,
    pub failures: Vec<DriveBindingFailure>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct VolumeDiskBinding {
    pub nt_path: String,
    pub disk_numbers: Vec<u32>,
}

pub(super) struct VolumeDiskBindings {
    pub bindings: Vec<VolumeDiskBinding>,
    pub failures: usize,
}

pub(super) struct DriveBindingFailure {
    pub label: String,
    pub stage: DriveBindingStage,
    pub error: io::Error,
}

#[derive(Clone, Copy)]
pub(super) enum DriveBindingStage {
    OpenVolume,
    QueryExtents,
}

pub(super) fn system_cpu_times() -> io::Result<CpuTimes> {
    let mut idle = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut kernel = idle;
    let mut user = idle;
    // SAFETY: all pointers refer to initialized writable FILETIME values.
    if unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: GetActiveProcessorCount has no pointer arguments and ALL_PROCESSOR_GROUPS is documented.
    let count = unsafe { GetActiveProcessorCount(ALL_PROCESSOR_GROUPS) } as usize;
    Ok(CpuTimes {
        idle: filetime(idle),
        kernel: filetime(kernel),
        user: filetime(user),
        logical_cpu_count: count,
    })
}

pub(super) fn memory_status() -> io::Result<MemoryStatus> {
    let mut value = MEMORYSTATUSEX {
        dwLength: size_of::<MEMORYSTATUSEX>() as u32,
        dwMemoryLoad: 0,
        ullTotalPhys: 0,
        ullAvailPhys: 0,
        ullTotalPageFile: 0,
        ullAvailPageFile: 0,
        ullTotalVirtual: 0,
        ullAvailVirtual: 0,
        ullAvailExtendedVirtual: 0,
    };
    // SAFETY: value is a correctly sized writable MEMORYSTATUSEX.
    if unsafe { GlobalMemoryStatusEx(&mut value) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(MemoryStatus {
        total_phys: value.ullTotalPhys,
        avail_phys: value.ullAvailPhys,
    })
}

#[derive(Default)]
struct PageFileAccumulator {
    total_pages: u64,
    used_pages: u64,
    overflowed: bool,
}

unsafe extern "system" fn accumulate_page_file(
    context: *mut core::ffi::c_void,
    info: *mut ENUM_PAGE_FILE_INFORMATION,
    _filename: windows_sys::core::PCWSTR,
) -> BOOL {
    // SAFETY: K32EnumPageFilesW invokes this callback with the context pointer supplied by
    // page_file_status and a valid ENUM_PAGE_FILE_INFORMATION for the duration of the call.
    let accumulator = unsafe { &mut *context.cast::<PageFileAccumulator>() };
    // SAFETY: info is supplied by K32EnumPageFilesW and remains valid for this callback.
    let info = unsafe { &*info };

    match accumulator.total_pages.checked_add(info.TotalSize as u64) {
        Some(total) => accumulator.total_pages = total,
        None => accumulator.overflowed = true,
    }
    match accumulator.used_pages.checked_add(info.TotalInUse as u64) {
        Some(used) => accumulator.used_pages = used,
        None => accumulator.overflowed = true,
    }
    1
}

pub(super) fn page_file_status() -> io::Result<PageFileStatus> {
    let mut system_info = SYSTEM_INFO::default();
    // SAFETY: system_info is a valid writable SYSTEM_INFO value.
    unsafe { GetSystemInfo(&mut system_info) };

    let mut accumulator = PageFileAccumulator::default();
    // SAFETY: the callback and context remain valid for the synchronous enumeration call.
    if unsafe {
        K32EnumPageFilesW(
            Some(accumulate_page_file),
            (&mut accumulator as *mut PageFileAccumulator).cast(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    if accumulator.overflowed {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "page-file page count overflow",
        ));
    }

    page_file_bytes(
        system_info.dwPageSize as u64,
        accumulator.total_pages,
        accumulator.used_pages,
    )
}

pub(super) fn page_file_bytes(
    page_size: u64,
    total_pages: u64,
    used_pages: u64,
) -> io::Result<PageFileStatus> {
    let total_bytes = total_pages.checked_mul(page_size).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "page-file total byte overflow")
    })?;
    let used_bytes = used_pages.checked_mul(page_size).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "page-file used byte overflow")
    })?;
    Ok(PageFileStatus {
        total_bytes,
        used_bytes,
    })
}

pub(super) fn system_processes() -> io::Result<Vec<NativeProcess>> {
    let mut buffer = vec![0_u8; 256 * 1024];
    loop {
        let mut required = 0_u32;
        // SAFETY: buffer is writable for its advertised length; return-length points to writable u32.
        let status = unsafe {
            NtQuerySystemInformation(
                SystemProcessInformation,
                buffer.as_mut_ptr().cast(),
                buffer.len() as u32,
                &mut required,
            )
        };
        if status == STATUS_INFO_LENGTH_MISMATCH {
            buffer.resize((required as usize).saturating_add(64 * 1024), 0);
            continue;
        }
        if status < 0 {
            return Err(io::Error::other(format!(
                "NTSTATUS 0x{:08x}",
                status as u32
            )));
        }
        break;
    }
    let mut result = Vec::new();
    let mut offset = 0_usize;
    loop {
        if offset + size_of::<SYSTEM_PROCESS_INFORMATION>() > buffer.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "truncated SYSTEM_PROCESS_INFORMATION",
            ));
        }
        // SAFETY: bounds checked above; Windows aligns every entry suitably for this structure.
        let row = unsafe {
            &*(buffer
                .as_ptr()
                .add(offset)
                .cast::<SYSTEM_PROCESS_INFORMATION>())
        };
        let reserved = &row.Reserved1;
        let create_time = i64_field(reserved, 24).max(0) as u64;
        let user_time = i64_field(reserved, 32).max(0) as u64;
        let kernel_time = i64_field(reserved, 40).max(0) as u64;
        let pid = row.UniqueProcessId as usize;
        let name = unicode_string(&row.ImageName).unwrap_or_else(|| {
            if pid == 0 {
                "System Idle Process".to_owned()
            } else {
                format!("pid-{pid}")
            }
        });
        result.push(NativeProcess {
            pid,
            name,
            create_time,
            user_time,
            kernel_time,
            working_set: row.WorkingSetSize,
        });
        if row.NextEntryOffset == 0 {
            break;
        }
        offset = offset
            .checked_add(row.NextEntryOffset as usize)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "process entry offset overflow")
            })?;
    }
    Ok(result)
}

pub(super) fn network_interfaces() -> io::Result<Vec<NativeNetwork>> {
    let mut table: *mut MIB_IF_TABLE2 = null_mut();
    // SAFETY: table is a writable output pointer; the returned allocation is freed below.
    let status = unsafe { GetIfTable2(&mut table) };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    if table.is_null() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "GetIfTable2 returned null table",
        ));
    }
    // SAFETY: successful GetIfTable2 returns a valid table with NumEntries contiguous MIB_IF_ROW2 entries.
    let rows = unsafe {
        std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize)
    };
    let result = rows
        .iter()
        .filter(|row| {
            row.OperStatus == IfOperStatusUp
                && row.InterfaceAndOperStatusFlags._bitfield & HARDWARE_INTERFACE_FLAG != 0
        })
        .map(|row| NativeNetwork {
            guid: guid_string(row),
            name: utf16_z(&row.Alias),
            rx_bytes: row.InOctets,
            tx_bytes: row.OutOctets,
        })
        .collect();
    // SAFETY: table was allocated by GetIfTable2 and is freed exactly once.
    unsafe { FreeMibTable(table.cast()) };
    Ok(result)
}

pub(super) fn network_identity_from_luid(luid: u64) -> io::Result<(String, String)> {
    let luid = NET_LUID_LH { Value: luid };
    let mut guid = windows_sys::core::GUID::default();
    // SAFETY: luid and guid are valid input/output objects for the documented conversion API.
    let status = unsafe { ConvertInterfaceLuidToGuid(&luid, &mut guid) };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }

    let mut alias = [0_u16; 257];
    // SAFETY: alias is writable for the supplied character count and luid is valid.
    let status = unsafe { ConvertInterfaceLuidToAlias(&luid, alias.as_mut_ptr(), alias.len()) };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }

    Ok((guid_key(&guid), utf16_z(&alias)))
}

pub(super) fn physical_disks() -> io::Result<NativeDisks> {
    let mut disks = Vec::new();
    let mut fallback_identities = 0;
    let mut first_error = None;
    for index in 0..MAX_PHYSICAL_DRIVES {
        let path = format!(r"\\.\PhysicalDrive{index}");
        let wide = wide_z(&path);
        // SAFETY: wide is nul-terminated; no security attributes/template; share flags permit coexistence.
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
            if !matches!(error.raw_os_error(), Some(2 | 3)) && first_error.is_none() {
                first_error = Some(error);
            }
            continue;
        }
        let performance = disk_performance(handle);
        let descriptor = storage_descriptor(handle);
        // SAFETY: handle was opened successfully and is closed exactly once here.
        unsafe { CloseHandle(handle) };
        let performance = match performance {
            Ok(performance) => performance,
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
                continue;
            }
        };
        let (identity, fallback) = descriptor.unwrap_or_else(|| {
            (
                format!("physical-drive:{}", performance.StorageDeviceNumber),
                true,
            )
        });
        fallback_identities += usize::from(fallback);
        disks.push(NativeDisk {
            disk_number: performance.StorageDeviceNumber,
            identity,
            read_bytes: performance.BytesRead.max(0) as u64,
            write_bytes: performance.BytesWritten.max(0) as u64,
        });
    }
    if disks.is_empty()
        && let Some(error) = first_error
    {
        return Err(error);
    }
    Ok(NativeDisks {
        disks,
        fallback_identities,
    })
}

pub(super) fn drive_bindings() -> io::Result<DriveBindings> {
    // SAFETY: GetLogicalDrives has no pointer arguments.
    let mask = unsafe { GetLogicalDrives() };
    if mask == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut bindings = Vec::new();
    let mut failures = Vec::new();
    for index in 0..26_u32 {
        if mask & (1_u32 << index) == 0 {
            continue;
        }
        let letter = (b'A' + index as u8) as char;
        let label = format!("{letter}:");
        let root = wide_z(&format!("{label}\\"));
        // SAFETY: root is a nul-terminated drive-root path.
        let drive_type = unsafe { GetDriveTypeW(root.as_ptr()) };
        if !matches!(drive_type, DRIVE_FIXED | DRIVE_REMOVABLE) {
            continue;
        }
        let path = format!(r"\\.\{label}");
        let wide = wide_z(&path);
        // SAFETY: wide is nul-terminated; zero desired access is sufficient for this metadata query.
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
            failures.push(DriveBindingFailure {
                label,
                stage: DriveBindingStage::OpenVolume,
                error: io::Error::last_os_error(),
            });
            continue;
        }
        let disk_numbers = volume_disk_numbers(handle);
        // SAFETY: handle was opened successfully and is closed exactly once here.
        unsafe { CloseHandle(handle) };
        match disk_numbers {
            Ok(disk_numbers) if !disk_numbers.is_empty() => {
                bindings.push(DriveBinding {
                    label,
                    disk_numbers,
                });
            }
            Ok(_) => {}
            Err(error) => failures.push(DriveBindingFailure {
                label,
                stage: DriveBindingStage::QueryExtents,
                error,
            }),
        }
    }
    Ok(DriveBindings { bindings, failures })
}

pub(super) fn volume_disk_bindings() -> io::Result<VolumeDiskBindings> {
    const VOLUME_NAME_CAPACITY: usize = 1024;
    const DEVICE_NAME_CAPACITY: usize = 4096;
    const ERROR_NO_MORE_FILES: i32 = 18;

    let mut volume_name = vec![0_u16; VOLUME_NAME_CAPACITY];
    // SAFETY: volume_name is writable for the advertised number of UTF-16 code units.
    let find = unsafe { FindFirstVolumeW(volume_name.as_mut_ptr(), volume_name.len() as u32) };
    if find == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }

    let mut bindings = Vec::new();
    let mut failures = 0_usize;
    let enumeration = loop {
        let Some(volume_guid_path) = utf16_buffer(&volume_name) else {
            break Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "volume enumeration returned non-terminated name",
            ));
        };
        match volume_disk_binding(&volume_guid_path, DEVICE_NAME_CAPACITY) {
            Ok(binding) => bindings.push(binding),
            Err(_) => failures = failures.saturating_add(1),
        }

        volume_name.fill(0);
        // SAFETY: find is a live volume-enumeration handle and volume_name is writable.
        if unsafe { FindNextVolumeW(find, volume_name.as_mut_ptr(), volume_name.len() as u32) } == 0
        {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(ERROR_NO_MORE_FILES) {
                break Ok(());
            }
            break Err(error);
        }
    };
    // SAFETY: find was returned by FindFirstVolumeW and is closed exactly once.
    unsafe { FindVolumeClose(find) };
    enumeration?;
    bindings.sort_by(|a, b| a.nt_path.cmp(&b.nt_path));
    bindings.dedup_by(|a, b| a.nt_path == b.nt_path && a.disk_numbers == b.disk_numbers);
    Ok(VolumeDiskBindings { bindings, failures })
}

fn volume_disk_binding(
    volume_guid_path: &str,
    device_name_capacity: usize,
) -> io::Result<VolumeDiskBinding> {
    let open_path = volume_guid_path.trim_end_matches('\\');
    let open_wide = wide_z(open_path);
    // SAFETY: open_wide is nul-terminated; zero desired access is sufficient for extent metadata.
    let handle = unsafe {
        CreateFileW(
            open_wide.as_ptr(),
            0,
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
    let disk_numbers = volume_disk_numbers(handle);
    // SAFETY: handle was opened successfully and is closed exactly once here.
    unsafe { CloseHandle(handle) };
    let disk_numbers = disk_numbers?;

    let dos_name = open_path
        .strip_prefix(r"\\?\")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid volume GUID path"))?;
    let dos_wide = wide_z(dos_name);
    let mut target = vec![0_u16; device_name_capacity];
    // SAFETY: dos_wide is nul-terminated and target is writable for the advertised size.
    let len =
        unsafe { QueryDosDeviceW(dos_wide.as_ptr(), target.as_mut_ptr(), target.len() as u32) };
    if len == 0 {
        return Err(io::Error::last_os_error());
    }
    let nt_path = utf16_buffer(&target[..len as usize]).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "QueryDosDevice returned non-terminated target",
        )
    })?;
    Ok(VolumeDiskBinding {
        nt_path,
        disk_numbers,
    })
}

fn utf16_buffer(buffer: &[u16]) -> Option<String> {
    let end = buffer.iter().position(|unit| *unit == 0)?;
    String::from_utf16(&buffer[..end]).ok()
}

fn volume_disk_numbers(handle: HANDLE) -> io::Result<Vec<u32>> {
    let mut buffer = vec![0_u8; 4096];
    let mut returned = 0_u32;
    // SAFETY: handle is valid and buffer is writable for its advertised size.
    if unsafe {
        DeviceIoControl(
            handle,
            IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS,
            null(),
            0,
            buffer.as_mut_ptr().cast(),
            buffer.len() as u32,
            &mut returned,
            null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    if (returned as usize) < size_of::<VOLUME_DISK_EXTENTS>() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "truncated VOLUME_DISK_EXTENTS",
        ));
    }
    // SAFETY: size checked above; the returned buffer starts with VOLUME_DISK_EXTENTS.
    let extents = unsafe { &*(buffer.as_ptr().cast::<VOLUME_DISK_EXTENTS>()) };
    let count = extents.NumberOfDiskExtents as usize;
    let extent_size = size_of_val(&extents.Extents[0]);
    let required = std::mem::offset_of!(VOLUME_DISK_EXTENTS, Extents)
        .saturating_add(count.saturating_mul(extent_size));
    if required > returned as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "truncated volume disk extents",
        ));
    }
    // SAFETY: required size was validated against the returned byte count; Extents is the first
    // element of the API's variable-length trailing DISK_EXTENT array.
    let rows = unsafe { std::slice::from_raw_parts(extents.Extents.as_ptr(), count) };
    let mut numbers = rows
        .iter()
        .map(|extent| extent.DiskNumber)
        .collect::<Vec<_>>();
    numbers.sort_unstable();
    numbers.dedup();
    Ok(numbers)
}

fn disk_performance(handle: HANDLE) -> io::Result<DISK_PERFORMANCE> {
    // SAFETY: zero is a valid initial representation for this POD output structure.
    let mut output: DISK_PERFORMANCE = unsafe { zeroed() };
    let mut returned = 0_u32;
    // SAFETY: handle is valid; output points to writable DISK_PERFORMANCE storage.
    if unsafe {
        DeviceIoControl(
            handle,
            IOCTL_DISK_PERFORMANCE,
            null(),
            0,
            (&mut output as *mut DISK_PERFORMANCE).cast(),
            size_of::<DISK_PERFORMANCE>() as u32,
            &mut returned,
            null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(output)
}

pub(super) fn storage_descriptor(handle: HANDLE) -> Option<(String, bool)> {
    let query = STORAGE_PROPERTY_QUERY {
        PropertyId: StorageDeviceProperty,
        QueryType: PropertyStandardQuery,
        AdditionalParameters: [0],
    };
    let mut buffer = vec![0_u8; 2048];
    let mut returned = 0_u32;
    // SAFETY: input/output buffers are valid for the stated sizes and handle is valid.
    if unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            (&query as *const STORAGE_PROPERTY_QUERY).cast(),
            size_of::<STORAGE_PROPERTY_QUERY>() as u32,
            buffer.as_mut_ptr().cast(),
            buffer.len() as u32,
            &mut returned,
            null_mut(),
        )
    } == 0
    {
        return None;
    }
    if (returned as usize) < size_of::<STORAGE_DEVICE_DESCRIPTOR>() {
        return None;
    }
    // SAFETY: size checked; STORAGE_DEVICE_DESCRIPTOR begins the returned buffer.
    let descriptor = unsafe { &*(buffer.as_ptr().cast::<STORAGE_DEVICE_DESCRIPTOR>()) };
    let serial = ansi_field(&buffer, descriptor.SerialNumberOffset);
    serial
        .filter(|s| !s.is_empty())
        .map(|serial| (format!("serial:{serial}"), false))
}

fn filetime(value: FILETIME) -> u64 {
    (u64::from(value.dwHighDateTime) << 32) | u64::from(value.dwLowDateTime)
}
fn i64_field(bytes: &[u8], offset: usize) -> i64 {
    let mut field = [0_u8; 8];
    field.copy_from_slice(&bytes[offset..offset + 8]);
    i64::from_ne_bytes(field)
}
fn unicode_string(value: &windows_sys::Win32::Foundation::UNICODE_STRING) -> Option<String> {
    if value.Buffer.is_null() || value.Length == 0 {
        return None;
    }
    let len = value.Length as usize / 2; /* SAFETY: UNICODE_STRING guarantees Buffer points to Length bytes for the lifetime of the query buffer. */
    Some(String::from_utf16_lossy(unsafe {
        std::slice::from_raw_parts(value.Buffer, len)
    }))
}
fn utf16_z(value: &[u16]) -> String {
    let len = value.iter().position(|c| *c == 0).unwrap_or(value.len());
    String::from_utf16_lossy(&value[..len])
}
fn wide_z(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
fn ansi_field(buffer: &[u8], offset: u32) -> Option<String> {
    let offset = offset as usize;
    if offset == 0 || offset >= buffer.len() {
        return None;
    }
    let tail = &buffer[offset..];
    let len = tail.iter().position(|b| *b == 0).unwrap_or(tail.len());
    Some(String::from_utf8_lossy(&tail[..len]).trim().to_owned())
}
fn guid_string(row: &MIB_IF_ROW2) -> String {
    guid_key(&row.InterfaceGuid)
}
fn guid_key(g: &windows_sys::core::GUID) -> String {
    format!(
        "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        g.data1,
        g.data2,
        g.data3,
        g.data4[0],
        g.data4[1],
        g.data4[2],
        g.data4[3],
        g.data4[4],
        g.data4[5],
        g.data4[6],
        g.data4[7]
    )
}
