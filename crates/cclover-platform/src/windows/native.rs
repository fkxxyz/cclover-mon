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

mod network;
mod process;
mod storage;
mod system;

pub(super) use network::{network_identity_from_luid, network_interfaces};
pub(super) use process::system_processes;
pub(super) use storage::{
    drive_bindings, physical_disks, storage_descriptor, volume_disk_bindings,
};
#[cfg(test)]
pub(super) use system::page_file_bytes;
pub(super) use system::{memory_status, page_file_status, system_cpu_times};

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
