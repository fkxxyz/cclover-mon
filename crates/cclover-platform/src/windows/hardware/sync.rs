#![allow(unsafe_code)]

use std::io;
use std::ptr::null;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0};
use windows_sys::Win32::System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject};

pub(super) struct NamedMutexGuard {
    handle: HANDLE,
}

pub(super) fn lock_named(name: &str, wait_ms: u32, label: &str) -> io::Result<NamedMutexGuard> {
    let name = wide_z(name);
    // SAFETY: name is nul-terminated; default security attributes are intentional.
    let handle = unsafe { CreateMutexW(null(), 0, name.as_ptr()) };
    if handle.is_null() {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: handle is a live mutex handle.
    let result = unsafe { WaitForSingleObject(handle, wait_ms) };
    if result != WAIT_OBJECT_0 && result != WAIT_ABANDONED {
        // SAFETY: handle was created above and is not owned after timeout/failure.
        unsafe { CloseHandle(handle) };
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            format!("timed out waiting for {label} mutex"),
        ));
    }
    Ok(NamedMutexGuard { handle })
}

impl Drop for NamedMutexGuard {
    fn drop(&mut self) {
        // SAFETY: guard owns a successfully acquired mutex handle.
        unsafe {
            ReleaseMutex(self.handle);
            CloseHandle(self.handle);
        }
    }
}

fn wide_z(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
