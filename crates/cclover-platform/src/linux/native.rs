#![allow(unsafe_code)]

use std::ffi::{CStr, CString};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

pub(super) fn numeric_directory_entries(path: &Path) -> io::Result<Vec<(u32, u64)>> {
    struct Directory(*mut libc::DIR);

    impl Drop for Directory {
        fn drop(&mut self) {
            // SAFETY: self.0 is a live DIR* returned by opendir and owned by this guard.
            unsafe { libc::closedir(self.0) };
        }
    }

    let path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains NUL"))?;
    // SAFETY: path is a valid NUL-terminated C string for the duration of the call.
    let dir = unsafe { libc::opendir(path.as_ptr()) };
    if dir.is_null() {
        return Err(io::Error::last_os_error());
    }
    let directory = Directory(dir);
    let mut entries = Vec::new();
    loop {
        // SAFETY: directory.0 remains live until the guard is dropped; readdir returns either
        // null or a pointer valid until the next readdir call on this DIR*.
        let entry = unsafe { libc::readdir(directory.0) };
        if entry.is_null() {
            break;
        }
        // SAFETY: d_name is guaranteed to be NUL-terminated by readdir.
        let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
        let Some(pid) = parse_u32_decimal(name) else {
            continue;
        };
        // SAFETY: entry points to the current live dirent returned by readdir.
        entries.push((pid, unsafe { (*entry).d_ino as u64 }));
    }
    Ok(entries)
}

fn parse_u32_decimal(bytes: &[u8]) -> Option<u32> {
    if bytes.is_empty() {
        return None;
    }
    let mut value = 0_u32;
    for byte in bytes {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value
            .checked_mul(10)?
            .checked_add(u32::from(*byte - b'0'))?;
    }
    Some(value)
}

pub(super) fn page_size() -> Option<u64> {
    // SAFETY: sysconf is a process-local query with no pointer arguments; _SC_PAGESIZE is valid.
    let value = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    (value > 0).then_some(value as u64)
}

#[cfg(any(feature = "ebpf-io", test))]
pub(super) fn clock_ticks_per_second() -> Option<u64> {
    // SAFETY: sysconf is a process-local query with no pointer arguments; _SC_CLK_TCK is valid.
    let value = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    (value > 0).then_some(value as u64)
}

#[cfg(feature = "ebpf-io")]
pub(super) fn interface_name(ifindex: u32) -> Option<String> {
    let mut name = [0_i8; libc::IF_NAMESIZE];
    // SAFETY: name points to IF_NAMESIZE writable bytes for the duration of the call.
    // if_indextoname either returns that same buffer pointer or null on failure.
    let ptr = unsafe { libc::if_indextoname(ifindex, name.as_mut_ptr()) };
    if ptr.is_null() {
        return None;
    }
    // SAFETY: successful if_indextoname writes a NUL-terminated interface name into name.
    Some(
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned(),
    )
}
