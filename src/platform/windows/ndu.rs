#![allow(dead_code)]
#![allow(unsafe_code)]

//! Narrow compatibility wrapper for Windows Network Data Usage (NDU) accounting.
//!
//! NDU is a built-in Windows facility (`Ndu.sys`) used by Windows data-usage/SRUM
//! components. Its device ABI is undocumented, so the constants and layouts below are
//! compatibility knowledge rather than SDK contracts.
//!
//! Current layout evidence is from Windows 11 build 26100: `nduprov.dll`
//! `DeviceIoControl` call sites, NDU ETW events 2004/2007, and controlled
//! `QUERY_STATS` captures correlated with known PID/IfLuid traffic. Revalidate these
//! observations when a supported Windows build changes the NDU ABI.

use std::io;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{CreateFileW, OPEN_EXISTING};
use windows_sys::Win32::System::IO::DeviceIoControl;

const DEVICE_PATH: &str = r"\\.\NduIoDevice";
const GENERIC_READ: u32 = 0x8000_0000;
const GENERIC_WRITE: u32 = 0x4000_0000;

// Observed in nduprov.dll. These are FILE_DEVICE_NETWORK, READ|WRITE access,
// METHOD_BUFFERED, with private function numbers 0, 1, and 2 respectively.
const IOCTL_NDU_START_STATS_COLLECTION: u32 = 0x0012_C000;
const IOCTL_NDU_STOP_STATS_COLLECTION: u32 = 0x0012_C004;
const IOCTL_NDU_QUERY_STATS: u32 = 0x0012_C008;

// QUERY_STATS returns a private 16-byte envelope before the self-relative snapshot.
// DWORD +0 is an NTSTATUS; QWORD +8 is the snapshot byte size. On
// STATUS_BUFFER_TOO_SMALL, nduprov.dll allocates 16 + that size and retries.
const QUERY_ENVELOPE_BYTES: usize = 16;
const INITIAL_QUERY_BYTES: usize = 0x810;
const MAX_QUERY_BYTES: usize = 64 * 1024 * 1024;
const STATUS_BUFFER_TOO_SMALL: u32 = 0xC000_0023;

// Observed snapshot layout. Pointers are unsigned self-relative byte offsets whose
// base is the address of the offset field itself, not the start of the snapshot.
const SNAPSHOT_HEADER_BYTES: usize = 0x28;
const SNAPSHOT_SIZE_OFFSET: usize = 0x00;
const INTERFACE_COUNT_OFFSET: usize = 0x08;
const INTERFACE_LIST_OFFSET: usize = 0x10;
const ATTRIBUTION_COUNT_OFFSET: usize = 0x18;
const ATTRIBUTION_LIST_OFFSET: usize = 0x20;

const INTERFACE_RECORD_BYTES: usize = 0x28;
const IF_LUID_OFFSET: usize = 0x00;
const PROFILE_ID_OFFSET: usize = 0x08;
const BYTES_SENT_OFFSET: usize = 0x10;
const BYTES_RECEIVED_OFFSET: usize = 0x18;

const ATTRIBUTION_RECORD_BYTES: usize = 0x48;
const ATTRIBUTION_PID_OFFSET: usize = 0x00;
const ATTRIBUTION_KIND_OFFSET: usize = 0x04;
const ATTRIBUTION_EXE_PATH_OFFSET: usize = 0x08;
const ATTRIBUTION_INTERFACE_COUNT_OFFSET: usize = 0x38;
const ATTRIBUTION_INTERFACE_LIST_OFFSET: usize = 0x40;

// Kind 2 is the normal process attribution shape observed from controlled curl.exe
// traffic. Other NDU attribution kinds are preserved numerically and are not guessed.
const ATTRIBUTION_KIND_PROCESS: u32 = 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct InterfaceUsage {
    pub if_luid: u64,
    pub profile_id: u32,
    pub tx_bytes: u64,
    pub rx_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ProcessAttribution {
    pub pid: u32,
    pub executable: Option<String>,
    pub interfaces: Vec<InterfaceUsage>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Snapshot {
    /// Whole-machine/interface accounting for the consumed NDU interval.
    pub interfaces: Vec<InterfaceUsage>,
    /// Process records whose observed kind/layout is understood by this wrapper.
    pub process_attributions: Vec<ProcessAttribution>,
    /// Records with other private NDU kinds, intentionally left uninterpreted.
    pub other_attribution_count: usize,
}

pub(super) struct Session {
    handle: HANDLE,
    started: bool,
}

// A Windows device HANDLE may be used after its owning object moves to another thread.
// Session has exclusive ownership and never shares concurrent access to the handle.
unsafe impl Send for Session {}

impl Session {
    pub(super) fn open() -> io::Result<Self> {
        let path = wide_z(DEVICE_PATH);
        let handle = unsafe {
            CreateFileW(
                path.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0,
                null(),
                OPEN_EXISTING,
                0,
                null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }

        let mut session = Self {
            handle,
            started: false,
        };
        if let Err(error) = session.control(IOCTL_NDU_START_STATS_COLLECTION) {
            return Err(error);
        }
        session.started = true;
        Ok(session)
    }

    /// Consumes the current NDU accounting interval; the next query reports only usage
    /// accumulated after this one.
    pub(super) fn query_interval(&self) -> io::Result<Snapshot> {
        let raw = self.query_raw()?;
        parse_snapshot(&raw)
    }

    fn control(&self, code: u32) -> io::Result<()> {
        let mut returned = 0_u32;
        if unsafe {
            DeviceIoControl(
                self.handle,
                code,
                null(),
                0,
                null_mut(),
                0,
                &mut returned,
                null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn query_raw(&self) -> io::Result<Vec<u8>> {
        let mut capacity = INITIAL_QUERY_BYTES;
        loop {
            let mut buffer = vec![0_u8; capacity];
            let mut returned = 0_u32;
            let output_size =
                u32::try_from(buffer.len()).map_err(|_| invalid("query buffer too large"))?;

            // QUERY_STATS uses the envelope's size rather than `bytesReturned` to locate
            // the self-relative snapshot, matching nduprov.dll behavior.
            if unsafe {
                DeviceIoControl(
                    self.handle,
                    IOCTL_NDU_QUERY_STATS,
                    null(),
                    0,
                    buffer.as_mut_ptr().cast(),
                    output_size,
                    &mut returned,
                    null_mut(),
                )
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
            if buffer.len() < QUERY_ENVELOPE_BYTES {
                return Err(invalid("NDU query envelope is truncated"));
            }

            let status = read_u32(&buffer, 0)?;
            let snapshot_size = usize_from_u64(read_u64(&buffer, 8)?)?;
            if status == STATUS_BUFFER_TOO_SMALL {
                let needed = QUERY_ENVELOPE_BYTES
                    .checked_add(snapshot_size)
                    .ok_or_else(|| invalid("NDU query size overflow"))?;
                if needed <= capacity || needed > MAX_QUERY_BYTES {
                    return Err(invalid("NDU returned an invalid retry size"));
                }
                capacity = needed;
                continue;
            }
            // NTSTATUS is signed; negative values have the high bit set. nduprov.dll also
            // accepts non-negative informational/success statuses.
            if status & 0x8000_0000 != 0 {
                return Err(invalid(format!(
                    "NDU query failed with NTSTATUS 0x{status:08X}"
                )));
            }

            let end = QUERY_ENVELOPE_BYTES
                .checked_add(snapshot_size)
                .ok_or_else(|| invalid("NDU query size overflow"))?;
            if snapshot_size < SNAPSHOT_HEADER_BYTES || end > buffer.len() {
                return Err(invalid("NDU query snapshot size is invalid"));
            }
            return Ok(buffer[QUERY_ENVELOPE_BYTES..end].to_vec());
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if self.started {
            let _ = self.control(IOCTL_NDU_STOP_STATS_COLLECTION);
            self.started = false;
        }
        unsafe { CloseHandle(self.handle) };
    }
}

fn parse_snapshot(bytes: &[u8]) -> io::Result<Snapshot> {
    if bytes.len() < SNAPSHOT_HEADER_BYTES {
        return Err(invalid("NDU snapshot header is truncated"));
    }

    // The validated schema repeats the envelope snapshot size here; mismatch indicates
    // truncation or a layout change before any self-relative offsets are followed.
    let declared_size = usize_from_u64(read_u64(bytes, SNAPSHOT_SIZE_OFFSET)?)?;
    if declared_size != bytes.len() {
        return Err(invalid("NDU snapshot size field does not match envelope"));
    }

    let interface_count = usize::from(read_u16(bytes, INTERFACE_COUNT_OFFSET)?);
    let interfaces = if interface_count == 0 {
        Vec::new()
    } else {
        let interface_start = relative_target(bytes, INTERFACE_LIST_OFFSET)?;
        parse_interface_records(bytes, interface_start, interface_count)?
    };

    let attribution_count = usize::from(read_u16(bytes, ATTRIBUTION_COUNT_OFFSET)?);
    let attribution_start = if attribution_count == 0 {
        0
    } else {
        let start = relative_target(bytes, ATTRIBUTION_LIST_OFFSET)?;
        let attribution_bytes = attribution_count
            .checked_mul(ATTRIBUTION_RECORD_BYTES)
            .ok_or_else(|| invalid("NDU attribution count overflow"))?;
        checked_range(bytes, start, attribution_bytes)?;
        start
    };

    let mut process_attributions = Vec::new();
    let mut other_attribution_count = 0_usize;
    for index in 0..attribution_count {
        let base = attribution_start + index * ATTRIBUTION_RECORD_BYTES;
        let kind = read_u32(bytes, base + ATTRIBUTION_KIND_OFFSET)?;
        if kind != ATTRIBUTION_KIND_PROCESS {
            other_attribution_count += 1;
            continue;
        }

        let pid = read_u32(bytes, base + ATTRIBUTION_PID_OFFSET)?;
        let path_field = base + ATTRIBUTION_EXE_PATH_OFFSET;
        let executable = if read_u64(bytes, path_field)? == 0 {
            None
        } else {
            Some(read_utf16_z(bytes, relative_target(bytes, path_field)?)?)
        };
        let nested_count = usize::from(read_u16(bytes, base + ATTRIBUTION_INTERFACE_COUNT_OFFSET)?);
        let interfaces = if nested_count == 0 {
            Vec::new()
        } else {
            let nested_start = relative_target(bytes, base + ATTRIBUTION_INTERFACE_LIST_OFFSET)?;
            parse_interface_records(bytes, nested_start, nested_count)?
        };

        process_attributions.push(ProcessAttribution {
            pid,
            executable,
            interfaces,
        });
    }

    Ok(Snapshot {
        interfaces,
        process_attributions,
        other_attribution_count,
    })
}

fn parse_interface_records(
    bytes: &[u8],
    start: usize,
    count: usize,
) -> io::Result<Vec<InterfaceUsage>> {
    let records_bytes = count
        .checked_mul(INTERFACE_RECORD_BYTES)
        .ok_or_else(|| invalid("NDU interface count overflow"))?;
    checked_range(bytes, start, records_bytes)?;

    let mut records = Vec::with_capacity(count);
    for index in 0..count {
        let base = start + index * INTERFACE_RECORD_BYTES;
        records.push(InterfaceUsage {
            if_luid: read_u64(bytes, base + IF_LUID_OFFSET)?,
            profile_id: read_u32(bytes, base + PROFILE_ID_OFFSET)?,
            tx_bytes: read_u64(bytes, base + BYTES_SENT_OFFSET)?,
            rx_bytes: read_u64(bytes, base + BYTES_RECEIVED_OFFSET)?,
        });
    }
    Ok(records)
}

fn relative_target(bytes: &[u8], field: usize) -> io::Result<usize> {
    let relative = usize_from_u64(read_u64(bytes, field)?)?;
    let target = field
        .checked_add(relative)
        .ok_or_else(|| invalid("NDU relative offset overflow"))?;
    if target >= bytes.len() {
        return Err(invalid("NDU relative offset is out of bounds"));
    }
    Ok(target)
}

fn read_utf16_z(bytes: &[u8], start: usize) -> io::Result<String> {
    if start >= bytes.len() {
        return Err(invalid("NDU UTF-16 string offset is out of bounds"));
    }
    let tail = &bytes[start..];
    let mut units = Vec::new();
    for pair in tail.chunks_exact(2) {
        let unit = u16::from_le_bytes([pair[0], pair[1]]);
        if unit == 0 {
            return String::from_utf16(&units)
                .map_err(|_| invalid("NDU executable path is invalid UTF-16"));
        }
        units.push(unit);
    }
    Err(invalid("NDU executable path is not NUL-terminated"))
}

fn read_u16(bytes: &[u8], offset: usize) -> io::Result<u16> {
    let value = checked_range(bytes, offset, 2)?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> io::Result<u32> {
    let value = checked_range(bytes, offset, 4)?;
    Ok(u32::from_le_bytes(
        value.try_into().expect("four-byte slice"),
    ))
}

fn read_u64(bytes: &[u8], offset: usize) -> io::Result<u64> {
    let value = checked_range(bytes, offset, 8)?;
    Ok(u64::from_le_bytes(
        value.try_into().expect("eight-byte slice"),
    ))
}

fn checked_range(bytes: &[u8], start: usize, len: usize) -> io::Result<&[u8]> {
    let end = start
        .checked_add(len)
        .ok_or_else(|| invalid("NDU range overflow"))?;
    bytes
        .get(start..end)
        .ok_or_else(|| invalid("NDU record range is out of bounds"))
}

fn usize_from_u64(value: u64) -> io::Result<usize> {
    usize::try_from(value).map_err(|_| invalid("NDU size does not fit usize"))
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn wide_z(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_observed_process_and_interface_shape() {
        let path = r"\device\harddiskvolume3\windows\system32\curl.exe";
        let path_bytes: Vec<u8> = path
            .encode_utf16()
            .chain(std::iter::once(0))
            .flat_map(u16::to_le_bytes)
            .collect();

        let interface_start = SNAPSHOT_HEADER_BYTES;
        let attribution_start = interface_start + INTERFACE_RECORD_BYTES;
        let nested_start = attribution_start + ATTRIBUTION_RECORD_BYTES;
        let path_start = nested_start + INTERFACE_RECORD_BYTES;
        let total = path_start + path_bytes.len();
        let mut bytes = vec![0_u8; total];

        put_u64(&mut bytes, SNAPSHOT_SIZE_OFFSET, total as u64);
        put_u16(&mut bytes, INTERFACE_COUNT_OFFSET, 1);
        put_relative(&mut bytes, INTERFACE_LIST_OFFSET, interface_start);
        put_u16(&mut bytes, ATTRIBUTION_COUNT_OFFSET, 1);
        put_relative(&mut bytes, ATTRIBUTION_LIST_OFFSET, attribution_start);

        put_interface(
            &mut bytes,
            interface_start,
            0x0006_0080_0700_0000,
            7,
            11,
            13,
        );

        put_u32(&mut bytes, attribution_start + ATTRIBUTION_PID_OFFSET, 8596);
        put_u32(
            &mut bytes,
            attribution_start + ATTRIBUTION_KIND_OFFSET,
            ATTRIBUTION_KIND_PROCESS,
        );
        put_relative(
            &mut bytes,
            attribution_start + ATTRIBUTION_EXE_PATH_OFFSET,
            path_start,
        );
        put_u16(
            &mut bytes,
            attribution_start + ATTRIBUTION_INTERFACE_COUNT_OFFSET,
            1,
        );
        put_relative(
            &mut bytes,
            attribution_start + ATTRIBUTION_INTERFACE_LIST_OFFSET,
            nested_start,
        );
        put_interface(
            &mut bytes,
            nested_start,
            0x0006_0080_0700_0000,
            0,
            274_801,
            6_803_535,
        );
        bytes[path_start..].copy_from_slice(&path_bytes);

        let parsed = parse_snapshot(&bytes).expect("fixture should parse");
        assert_eq!(parsed.interfaces.len(), 1);
        assert_eq!(parsed.process_attributions.len(), 1);
        assert_eq!(parsed.other_attribution_count, 0);
        assert_eq!(parsed.process_attributions[0].pid, 8596);
        assert_eq!(
            parsed.process_attributions[0].executable.as_deref(),
            Some(path)
        );
        assert_eq!(
            parsed.process_attributions[0].interfaces[0].rx_bytes,
            6_803_535
        );
        assert_eq!(
            parsed.process_attributions[0].interfaces[0].tx_bytes,
            274_801
        );
    }

    #[test]
    fn ignores_unrecognized_attribution_layout_instead_of_following_process_offsets() {
        let attribution_start = SNAPSHOT_HEADER_BYTES;
        let total = attribution_start + ATTRIBUTION_RECORD_BYTES;
        let mut bytes = vec![0_u8; total];
        put_u64(&mut bytes, SNAPSHOT_SIZE_OFFSET, total as u64);
        put_u16(&mut bytes, INTERFACE_COUNT_OFFSET, 0);
        put_u64(&mut bytes, INTERFACE_LIST_OFFSET, u64::MAX);
        put_u16(&mut bytes, ATTRIBUTION_COUNT_OFFSET, 1);
        put_relative(&mut bytes, ATTRIBUTION_LIST_OFFSET, attribution_start);
        put_u32(&mut bytes, attribution_start + ATTRIBUTION_KIND_OFFSET, 3);
        put_u64(
            &mut bytes,
            attribution_start + ATTRIBUTION_INTERFACE_LIST_OFFSET,
            u64::MAX,
        );

        let parsed = parse_snapshot(&bytes).expect("unknown kind stays opaque");
        assert!(parsed.interfaces.is_empty());
        assert!(parsed.process_attributions.is_empty());
        assert_eq!(parsed.other_attribution_count, 1);
    }

    #[test]
    fn zero_counts_do_not_follow_unused_relative_offsets() {
        let mut bytes = vec![0_u8; SNAPSHOT_HEADER_BYTES];
        let total = bytes.len() as u64;
        put_u64(&mut bytes, SNAPSHOT_SIZE_OFFSET, total);
        put_u16(&mut bytes, INTERFACE_COUNT_OFFSET, 0);
        put_u64(&mut bytes, INTERFACE_LIST_OFFSET, u64::MAX);
        put_u16(&mut bytes, ATTRIBUTION_COUNT_OFFSET, 0);
        put_u64(&mut bytes, ATTRIBUTION_LIST_OFFSET, u64::MAX);

        let parsed = parse_snapshot(&bytes).expect("empty lists need no pointer target");
        assert!(parsed.interfaces.is_empty());
        assert!(parsed.process_attributions.is_empty());
        assert_eq!(parsed.other_attribution_count, 0);
    }

    #[test]
    fn rejects_out_of_bounds_self_relative_pointer() {
        let mut bytes = vec![0_u8; SNAPSHOT_HEADER_BYTES];
        let snapshot_len = bytes.len() as u64;
        put_u64(&mut bytes, SNAPSHOT_SIZE_OFFSET, snapshot_len);
        put_u16(&mut bytes, INTERFACE_COUNT_OFFSET, 1);
        put_u64(&mut bytes, INTERFACE_LIST_OFFSET, u64::MAX);
        put_u16(&mut bytes, ATTRIBUTION_COUNT_OFFSET, 0);
        put_u64(&mut bytes, ATTRIBUTION_LIST_OFFSET, 0);

        let error = parse_snapshot(&bytes).expect_err("invalid offset must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    fn put_interface(bytes: &mut [u8], base: usize, luid: u64, profile: u32, tx: u64, rx: u64) {
        put_u64(bytes, base + IF_LUID_OFFSET, luid);
        put_u32(bytes, base + PROFILE_ID_OFFSET, profile);
        put_u64(bytes, base + BYTES_SENT_OFFSET, tx);
        put_u64(bytes, base + BYTES_RECEIVED_OFFSET, rx);
    }

    fn put_relative(bytes: &mut [u8], field: usize, target: usize) {
        put_u64(bytes, field, (target - field) as u64);
    }

    fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
}
