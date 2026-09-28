#![allow(unsafe_code)]

#[cfg(feature = "ebpf-io")]
use std::ffi::CStr;

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
