#![allow(unsafe_code)]

// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// https://mozilla.org/MPL/2.0/.
// Copyright (C) LibreHardwareMonitor and Contributors.
// Partial Copyright (C) Michael Möller <mmoeller@openhardwaremonitor.org> and Contributors.
// Derived in part for cclover-mon from LibreHardwareMonitor LpcPort/LpcIO at reviewed commit
// 677a3a56abde9adff5abdb42db3a7638c9137572.

use std::io;
use std::ptr::null;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0};
use windows_sys::Win32::System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject};

use crate::platform::windows::pawnio::Session;

const ISA_MUTEX: &str = "Global\\Access_ISABUS.HTP.Method";
const ISA_WAIT_MS: u32 = 100;

pub(super) struct Access<'a> {
    session: &'a Session,
}

pub(super) struct IsaBusGuard {
    handle: HANDLE,
}

impl<'a> Access<'a> {
    pub(super) fn new(session: &'a Session) -> Self {
        Self { session }
    }

    pub(super) fn lock_isa_bus() -> io::Result<IsaBusGuard> {
        let name = wide_z(ISA_MUTEX);
        // SAFETY: name is nul-terminated; default security attributes are intentional.
        let handle = unsafe { CreateMutexW(null(), 0, name.as_ptr()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: handle is a live mutex handle.
        let result = unsafe { WaitForSingleObject(handle, ISA_WAIT_MS) };
        if result != WAIT_OBJECT_0 && result != WAIT_ABANDONED {
            // SAFETY: handle was created above and is not owned after timeout/failure.
            unsafe { CloseHandle(handle) };
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "timed out waiting for ISA/LPC bus mutex",
            ));
        }
        Ok(IsaBusGuard { handle })
    }

    pub(super) fn select_slot(&self, slot: u64) -> io::Result<()> {
        self.execute0("ioctl_select_slot", &[slot])
    }

    pub(super) fn find_bars(&self) -> io::Result<()> {
        self.execute0("ioctl_find_bars", &[])
    }

    pub(super) fn read_port(&self, port: u16) -> io::Result<u8> {
        Ok(self.execute1("ioctl_pio_inb", &[u64::from(port)])? as u8)
    }

    pub(super) fn write_port(&self, port: u16, value: u8) -> io::Result<()> {
        self.execute0("ioctl_pio_outb", &[u64::from(port), u64::from(value)])
    }

    pub(super) fn read_config_byte(&self, register: u8) -> io::Result<u8> {
        Ok(self.execute1("ioctl_superio_inb", &[u64::from(register)])? as u8)
    }

    pub(super) fn read_config_word(&self, register: u8) -> io::Result<u16> {
        Ok(self.execute1("ioctl_superio_inw", &[u64::from(register)])? as u16)
    }

    pub(super) fn write_config_byte(&self, register: u8, value: u8) -> io::Result<()> {
        self.execute0(
            "ioctl_superio_outb",
            &[u64::from(register), u64::from(value)],
        )
    }

    fn execute0(&self, name: &str, input: &[u64]) -> io::Result<()> {
        self.session.execute(name, input, 0).map(|_| ())
    }

    fn execute1(&self, name: &str, input: &[u64]) -> io::Result<u64> {
        self.session
            .execute(name, input, 1)?
            .first()
            .copied()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "PawnIO returned no value"))
    }
}

impl Drop for IsaBusGuard {
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
