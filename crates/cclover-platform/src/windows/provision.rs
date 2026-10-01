#![allow(unsafe_code)]

#[cfg(target_arch = "x86_64")]
use std::io;
#[cfg(target_arch = "x86_64")]
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::sync::OnceLock;

use windows_sys::Wdk::System::SystemServices::RtlGetVersion;
#[cfg(target_arch = "x86_64")]
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    DICD_GENERATE_ID, DIF_REGISTERDEVICE, DiInstallDriverW, SetupDiCallClassInstaller,
    SetupDiCreateDeviceInfoList, SetupDiCreateDeviceInfoW, SetupDiRemoveDevice,
    SetupDiSetDeviceRegistryPropertyW,
};
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    DIGCF_ALLCLASSES, DIGCF_PRESENT, HDEVINFO, SP_DEVINFO_DATA, SPDRP_HARDWAREID,
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo, SetupDiGetClassDevsW,
    SetupDiGetDevicePropertyW, SetupDiGetDeviceRegistryPropertyW,
};
use windows_sys::Win32::Devices::Properties::{DEVPKEY_Device_DriverVersion, DEVPROP_TYPE_STRING};
#[cfg(target_arch = "x86_64")]
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_CANCELLED, ERROR_SUCCESS_REBOOT_REQUIRED, HANDLE, LocalFree, WAIT_OBJECT_0,
};
use windows_sys::Win32::Foundation::{ERROR_NO_MORE_ITEMS, GetLastError};
#[cfg(target_arch = "x86_64")]
use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
#[cfg(target_arch = "x86_64")]
use windows_sys::Win32::Security::Cryptography::{
    BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom,
};
#[cfg(target_arch = "x86_64")]
use windows_sys::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
#[cfg(target_arch = "x86_64")]
use windows_sys::Win32::Storage::FileSystem::CreateDirectoryW;
use windows_sys::Win32::System::SystemInformation::OSVERSIONINFOW;
#[cfg(target_arch = "x86_64")]
use windows_sys::Win32::System::Threading::{
    CreateMutexW, GetExitCodeProcess, INFINITE, WaitForSingleObject,
};
#[cfg(target_arch = "x86_64")]
use windows_sys::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
#[cfg(target_arch = "x86_64")]
use windows_sys::core::GUID;

#[cfg(target_arch = "x86_64")]
use super::pawnio::embedded_driver_package;

const HELPER_ARG: &str = "--internal-provision-pawnio";
const HARDWARE_ID: &str = r"Root\PawnIO";
#[cfg(target_arch = "x86_64")]
const DEVICE_NAME: &str = "PawnIO";
#[cfg(target_arch = "x86_64")]
const MUTEX_NAME: &str = r"Global\cclover-mon-pawnio-provision-v1";
#[cfg(target_arch = "x86_64")]
const SDDL_REVISION_1: u32 = 1;
#[cfg(target_arch = "x86_64")]
const PRIVATE_DIRECTORY_SDDL: &str = "D:P(A;;FA;;;SY)(A;;FA;;;BA)";
#[cfg(target_arch = "x86_64")]
const EXIT_READY: u32 = 0;
const EXIT_FAILED: u32 = 1;
#[cfg(target_arch = "x86_64")]
const EXIT_REBOOT_REQUIRED: u32 = ERROR_SUCCESS_REBOOT_REQUIRED;
#[cfg(target_arch = "x86_64")]
const PAWNIO_CLASS_GUID: GUID = GUID::from_u128(0x62f9c741_b25a_46ce_b54c_9bccce08b6f2);

static MACHINE_STATUS: OnceLock<MachineStatus> = OnceLock::new();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MachineStatus {
    Ready,
    Unsupported,
    #[cfg(not(target_arch = "x86_64"))]
    ProvisioningUnsupported,
    #[cfg(target_arch = "x86_64")]
    ElevationDeclined,
    #[cfg(target_arch = "x86_64")]
    RebootRequired,
    InstallFailed,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Version([u16; 4]);

pub(super) fn prepare_machine_capability() -> MachineStatus {
    *MACHINE_STATUS.get_or_init(prepare_machine_capability_inner)
}

pub(super) fn machine_status() -> Option<MachineStatus> {
    MACHINE_STATUS.get().copied()
}

pub(super) fn early_command_exit_code() -> Option<i32> {
    if std::env::args().nth(1).as_deref() == Some(HELPER_ARG) {
        #[cfg(target_arch = "x86_64")]
        {
            return Some(match elevated_install() {
                Ok(true) => EXIT_REBOOT_REQUIRED as i32,
                Ok(false) => EXIT_READY as i32,
                Err(_) => EXIT_FAILED as i32,
            });
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            return Some(EXIT_FAILED as i32);
        }
    }

    None
}

fn prepare_machine_capability_inner() -> MachineStatus {
    prepare_machine_capability_with(&SystemEnvironment)
}

mod inspection;
mod install;
mod policy;

#[cfg(test)]
use inspection::parse_version;
use inspection::{
    installed_driver_version, minimum_windows_build, pawnio_device_present,
    required_driver_version, windows_build,
};
#[cfg(all(test, target_arch = "x86_64"))]
use install::wide_multi_z;
use install::{DeviceInfoSet, devinfo_data, multi_sz, wide_z};
#[cfg(target_arch = "x86_64")]
use install::{elevated_install, launch_elevated_helper};
#[cfg(test)]
use policy::ProvisionEnvironment;
use policy::{SystemEnvironment, prepare_machine_capability_with};

#[cfg(test)]
mod tests;
