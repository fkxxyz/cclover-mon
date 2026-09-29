#![allow(unsafe_code)]

use std::io;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::IO::DeviceIoControl;

const DEVICE_PATH: &str = r"\\?\GLOBALROOT\Device\PawnIO";
const GENERIC_READ: u32 = 0x8000_0000;
const GENERIC_WRITE: u32 = 0x4000_0000;
const IOCTL_PIO_LOAD_BINARY: u32 = 0xA1B2_2084;
const IOCTL_PIO_EXECUTE_FN: u32 = 0xA1B2_2104;
const FUNCTION_NAME_BYTES: usize = 32;

#[derive(Debug)]
pub(super) enum OpenError {
    NotInstalled,
    PermissionDenied,
    Other,
}

#[cfg(target_arch = "x86_64")]
pub(super) struct EmbeddedDriverPackage {
    pub inf: &'static [u8],
    pub sys: &'static [u8],
    pub cat: &'static [u8],
}

#[cfg(target_arch = "x86_64")]
const DRIVER_INF: &[u8] = include_bytes!(concat!(env!("CCLOVER_PAWNIO_DRIVER_DIR"), "/PawnIO.inf"));
#[cfg(target_arch = "x86_64")]
const DRIVER_SYS: &[u8] = include_bytes!(concat!(env!("CCLOVER_PAWNIO_DRIVER_DIR"), "/PawnIO.sys"));
#[cfg(target_arch = "x86_64")]
const DRIVER_CAT: &[u8] = include_bytes!(concat!(env!("CCLOVER_PAWNIO_DRIVER_DIR"), "/PawnIO.cat"));

#[cfg(target_arch = "x86_64")]
pub(super) fn embedded_driver_package() -> EmbeddedDriverPackage {
    EmbeddedDriverPackage {
        inf: DRIVER_INF,
        sys: DRIVER_SYS,
        cat: DRIVER_CAT,
    }
}

pub(super) struct Session {
    handle: HANDLE,
}

// SAFETY: a PawnIO file HANDLE is not thread-affine. Session has unique ownership and
// performs no unsynchronized interior mutation beyond kernel-managed handle state.
unsafe impl Send for Session {}

impl Session {
    pub(super) fn open() -> Result<Self, OpenError> {
        let path = wide_z(DEVICE_PATH);
        // SAFETY: path is nul-terminated; remaining pointer parameters are null by contract.
        let handle = unsafe {
            CreateFileW(
                path.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                null(),
                OPEN_EXISTING,
                0,
                null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            let error = io::Error::last_os_error();
            return match error.raw_os_error() {
                Some(2 | 3) => Err(OpenError::NotInstalled),
                Some(5) => Err(OpenError::PermissionDenied),
                _ => Err(OpenError::Other),
            };
        }
        Ok(Self { handle })
    }

    pub(super) fn load_module(&self, blob: &[u8]) -> io::Result<()> {
        let size = u32::try_from(blob.len()).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, "PawnIO module is too large")
        })?;
        let mut returned = 0_u32;
        // SAFETY: self.handle is live; blob is readable for size bytes; this IOCTL has no output buffer.
        if unsafe {
            DeviceIoControl(
                self.handle,
                IOCTL_PIO_LOAD_BINARY,
                blob.as_ptr().cast(),
                size,
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

    pub(super) fn execute(
        &self,
        name: &str,
        input: &[u64],
        output_len: usize,
    ) -> io::Result<Vec<u64>> {
        if name.as_bytes().contains(&0) || name.len() >= FUNCTION_NAME_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid PawnIO function name",
            ));
        }
        let input_bytes = input
            .len()
            .checked_mul(std::mem::size_of::<u64>())
            .and_then(|bytes| bytes.checked_add(FUNCTION_NAME_BYTES))
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "PawnIO input is too large")
            })?;
        let input_size = u32::try_from(input_bytes).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, "PawnIO input is too large")
        })?;
        let output_bytes = output_len
            .checked_mul(std::mem::size_of::<u64>())
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "PawnIO output is too large")
            })?;
        let output_size = u32::try_from(output_bytes).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, "PawnIO output is too large")
        })?;

        let mut request = vec![0_u8; input_bytes];
        request[..name.len()].copy_from_slice(name.as_bytes());
        for (index, value) in input.iter().enumerate() {
            let start = FUNCTION_NAME_BYTES + index * std::mem::size_of::<u64>();
            request[start..start + 8].copy_from_slice(&value.to_le_bytes());
        }
        let mut output = vec![0_u64; output_len];
        let mut returned = 0_u32;
        // SAFETY: handle is live; request/output buffers are valid for the advertised lengths.
        if unsafe {
            DeviceIoControl(
                self.handle,
                IOCTL_PIO_EXECUTE_FN,
                request.as_ptr().cast(),
                input_size,
                output.as_mut_ptr().cast(),
                output_size,
                &mut returned,
                null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        if returned % 8 != 0 || returned > output_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "PawnIO returned an invalid output length",
            ));
        }
        output.truncate(returned as usize / 8);
        Ok(output)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: Session uniquely owns this handle and closes it exactly once.
        unsafe { CloseHandle(self.handle) };
    }
}

fn wide_z(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
