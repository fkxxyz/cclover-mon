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

pub(crate) fn early_command_exit_code() -> Option<i32> {
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

trait ProvisionEnvironment {
    fn windows_build(&self) -> Option<u32>;
    fn installed_driver_version(&self) -> Option<Version>;
    fn pawnio_device_present(&self) -> bool;
    #[cfg(target_arch = "x86_64")]
    fn launch_elevated_helper(&self) -> io::Result<u32>;
}

struct SystemEnvironment;

impl ProvisionEnvironment for SystemEnvironment {
    fn windows_build(&self) -> Option<u32> {
        windows_build()
    }

    fn installed_driver_version(&self) -> Option<Version> {
        installed_driver_version()
    }

    fn pawnio_device_present(&self) -> bool {
        pawnio_device_present()
    }

    #[cfg(target_arch = "x86_64")]
    fn launch_elevated_helper(&self) -> io::Result<u32> {
        launch_elevated_helper()
    }
}

fn prepare_machine_capability_with(environment: &impl ProvisionEnvironment) -> MachineStatus {
    if environment
        .windows_build()
        .is_none_or(|build| build < minimum_windows_build())
    {
        return MachineStatus::Unsupported;
    }

    let required = required_driver_version();
    if environment
        .installed_driver_version()
        .is_some_and(|installed| installed >= required)
    {
        return MachineStatus::Ready;
    }
    if environment.installed_driver_version().is_none() && environment.pawnio_device_present() {
        return MachineStatus::InstallFailed;
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        return MachineStatus::ProvisioningUnsupported;
    }
    #[cfg(target_arch = "x86_64")]
    {
        match environment.launch_elevated_helper() {
            Ok(EXIT_READY) => {
                if environment
                    .installed_driver_version()
                    .is_some_and(|installed| installed >= required)
                {
                    MachineStatus::Ready
                } else {
                    MachineStatus::InstallFailed
                }
            }
            Ok(EXIT_REBOOT_REQUIRED) => MachineStatus::RebootRequired,
            Err(error) if error.raw_os_error() == Some(ERROR_CANCELLED as i32) => {
                MachineStatus::ElevationDeclined
            }
            _ => MachineStatus::InstallFailed,
        }
    }
}

fn required_driver_version() -> Version {
    parse_version(env!("CCLOVER_PAWNIO_DRIVER_VERSION")).expect("prepared PawnIO version is valid")
}

fn minimum_windows_build() -> u32 {
    env!("CCLOVER_PAWNIO_MIN_WINDOWS_BUILD")
        .parse()
        .expect("prepared PawnIO minimum Windows build is valid")
}

fn windows_build() -> Option<u32> {
    let mut version = OSVERSIONINFOW {
        dwOSVersionInfoSize: size_of::<OSVERSIONINFOW>() as u32,
        ..Default::default()
    };
    // SAFETY: version points to a correctly sized writable OSVERSIONINFOW.
    let status = unsafe { RtlGetVersion(&mut version) };
    (status >= 0).then_some(version.dwBuildNumber)
}

fn installed_driver_version() -> Option<Version> {
    let root = wide_z("ROOT");
    // SAFETY: arguments are valid and the enumerator string is nul-terminated.
    let set = unsafe {
        SetupDiGetClassDevsW(
            null(),
            root.as_ptr(),
            null_mut(),
            DIGCF_ALLCLASSES | DIGCF_PRESENT,
        )
    };
    if set == -1_isize {
        return None;
    }
    let set = DeviceInfoSet(set);

    let mut index = 0;
    loop {
        let mut info = devinfo_data();
        // SAFETY: set is live and info is correctly initialized.
        if unsafe { SetupDiEnumDeviceInfo(set.0, index, &mut info) } == 0 {
            // SAFETY: immediately reads the thread-local error from the failed API call.
            if unsafe { GetLastError() } == ERROR_NO_MORE_ITEMS {
                return None;
            }
            return None;
        }
        index += 1;
        if !device_has_pawnio_hardware_id(set.0, &mut info) {
            continue;
        }
        return device_driver_version(set.0, &info);
    }
}

fn pawnio_device_present() -> bool {
    let root = wide_z("ROOT");
    // SAFETY: arguments are valid and the enumerator string is nul-terminated.
    let set = unsafe {
        SetupDiGetClassDevsW(
            null(),
            root.as_ptr(),
            null_mut(),
            DIGCF_ALLCLASSES | DIGCF_PRESENT,
        )
    };
    if set == -1_isize {
        return false;
    }
    let set = DeviceInfoSet(set);
    let mut index = 0;
    loop {
        let mut info = devinfo_data();
        // SAFETY: set is live and info is correctly initialized.
        if unsafe { SetupDiEnumDeviceInfo(set.0, index, &mut info) } == 0 {
            return false;
        }
        index += 1;
        if device_has_pawnio_hardware_id(set.0, &mut info) {
            return true;
        }
    }
}

fn device_has_pawnio_hardware_id(set: HDEVINFO, info: &mut SP_DEVINFO_DATA) -> bool {
    let mut wide = [0_u16; 256];
    let mut required = 0_u32;
    // SAFETY: all pointers reference writable/readable buffers for the supplied sizes.
    if unsafe {
        SetupDiGetDeviceRegistryPropertyW(
            set,
            info,
            SPDRP_HARDWAREID,
            null_mut(),
            wide.as_mut_ptr().cast(),
            size_of_val(&wide) as u32,
            &mut required,
        )
    } == 0
    {
        return false;
    }
    let used = (required as usize / 2).min(wide.len());
    multi_sz(&wide[..used]).any(|id| id.eq_ignore_ascii_case(HARDWARE_ID))
}

fn device_driver_version(set: HDEVINFO, info: &SP_DEVINFO_DATA) -> Option<Version> {
    let mut property_type = 0_u32;
    let mut buffer = [0_u16; 64];
    let mut required = 0_u32;
    // SAFETY: buffer is writable for its advertised byte length and key/info are valid.
    if unsafe {
        SetupDiGetDevicePropertyW(
            set,
            info,
            &DEVPKEY_Device_DriverVersion,
            &mut property_type,
            buffer.as_mut_ptr().cast(),
            size_of_val(&buffer) as u32,
            &mut required,
            0,
        )
    } == 0
        || property_type != DEVPROP_TYPE_STRING
    {
        return None;
    }
    let len = buffer
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(buffer.len());
    parse_version(&String::from_utf16_lossy(&buffer[..len]))
}

fn parse_version(value: &str) -> Option<Version> {
    let mut parts = [0_u16; 4];
    let mut seen = 0;
    for (index, part) in value.split('.').enumerate() {
        if index >= parts.len() {
            return None;
        }
        parts[index] = part.parse().ok()?;
        seen += 1;
    }
    (seen >= 2).then_some(Version(parts))
}

#[cfg(target_arch = "x86_64")]
fn launch_elevated_helper() -> io::Result<u32> {
    let exe = std::env::current_exe()?;
    let exe = wide_z(&exe.to_string_lossy());
    let verb = wide_z("runas");
    let args = wide_z(HELPER_ARG);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: verb.as_ptr(),
        lpFile: exe.as_ptr(),
        lpParameters: args.as_ptr(),
        nShow: 0,
        ..Default::default()
    };
    // SAFETY: strings remain alive across the synchronous ShellExecuteExW call.
    if unsafe { ShellExecuteExW(&mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if info.hProcess.is_null() {
        return Err(io::Error::other(
            "elevated PawnIO helper returned no process handle",
        ));
    }
    let process = OwnedHandle(info.hProcess);
    // SAFETY: process is a live process handle.
    if unsafe { WaitForSingleObject(process.0, INFINITE) } != WAIT_OBJECT_0 {
        return Err(io::Error::last_os_error());
    }
    let mut exit_code = 0_u32;
    // SAFETY: process is live and exit_code is writable.
    if unsafe { GetExitCodeProcess(process.0, &mut exit_code) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(exit_code)
}

#[cfg(target_arch = "x86_64")]
fn elevated_install() -> io::Result<bool> {
    let mutex_name = wide_z(MUTEX_NAME);
    // SAFETY: mutex name is nul-terminated and security attributes are intentionally default.
    let mutex = unsafe { CreateMutexW(null(), 0, mutex_name.as_ptr()) };
    if mutex.is_null() {
        return Err(io::Error::last_os_error());
    }
    let mutex = OwnedHandle(mutex);
    // SAFETY: mutex is a live synchronization handle.
    if unsafe { WaitForSingleObject(mutex.0, INFINITE) } != WAIT_OBJECT_0 {
        return Err(io::Error::last_os_error());
    }

    if installed_driver_version().is_some_and(|installed| installed >= required_driver_version()) {
        return Ok(false);
    }
    if installed_driver_version().is_none() && pawnio_device_present() {
        return Err(io::Error::other(
            "existing PawnIO device has no readable driver version; refusing to overwrite it",
        ));
    }

    let staging = PrivateStaging::create()?;
    let package = embedded_driver_package();
    std::fs::write(staging.path.join("PawnIO.inf"), package.inf)?;
    std::fs::write(staging.path.join("PawnIO.sys"), package.sys)?;
    std::fs::write(staging.path.join("PawnIO.cat"), package.cat)?;

    let mut created = None;
    if !pawnio_device_present() {
        created = Some(create_root_device()?);
    }

    let inf = wide_z(&staging.path.join("PawnIO.inf").to_string_lossy());
    let mut reboot = 0;
    // SAFETY: INF path is absolute/nul-terminated and points to the private staged package.
    let installed = unsafe { DiInstallDriverW(null_mut(), inf.as_ptr(), 0, &mut reboot) };
    if installed == 0 {
        if let Some(mut device) = created {
            device.remove_best_effort();
        }
        return Err(io::Error::last_os_error());
    }
    Ok(reboot != 0)
}

#[cfg(target_arch = "x86_64")]
fn create_root_device() -> io::Result<CreatedDevice> {
    // SAFETY: class GUID is static and HWND is intentionally null.
    let set = unsafe { SetupDiCreateDeviceInfoList(&PAWNIO_CLASS_GUID, null_mut()) };
    if set == -1_isize {
        return Err(io::Error::last_os_error());
    }
    let set = DeviceInfoSet(set);
    let mut info = devinfo_data();
    let name = wide_z(DEVICE_NAME);
    // SAFETY: all input pointers are valid for this call and info is writable.
    if unsafe {
        SetupDiCreateDeviceInfoW(
            set.0,
            name.as_ptr(),
            &PAWNIO_CLASS_GUID,
            null(),
            null_mut(),
            DICD_GENERATE_ID,
            &mut info,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }

    let hardware_id = wide_multi_z(HARDWARE_ID);
    // SAFETY: property data is a valid UTF-16 REG_MULTI_SZ for the advertised byte length.
    if unsafe {
        SetupDiSetDeviceRegistryPropertyW(
            set.0,
            &mut info,
            SPDRP_HARDWAREID,
            hardware_id.as_ptr().cast(),
            size_of_val(hardware_id.as_slice()) as u32,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: newly created device info is valid for registration.
    if unsafe { SetupDiCallClassInstaller(DIF_REGISTERDEVICE, set.0, &info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(CreatedDevice { set, info })
}

#[cfg(target_arch = "x86_64")]
struct CreatedDevice {
    set: DeviceInfoSet,
    info: SP_DEVINFO_DATA,
}

#[cfg(target_arch = "x86_64")]
impl CreatedDevice {
    fn remove_best_effort(&mut self) {
        // SAFETY: this devnode was created by this provisioning attempt.
        unsafe { SetupDiRemoveDevice(self.set.0, &mut self.info) };
    }
}

struct DeviceInfoSet(HDEVINFO);

impl Drop for DeviceInfoSet {
    fn drop(&mut self) {
        // SAFETY: this object uniquely owns the SetupAPI list handle.
        unsafe { SetupDiDestroyDeviceInfoList(self.0) };
    }
}

#[cfg(target_arch = "x86_64")]
struct PrivateStaging {
    path: PathBuf,
}

#[cfg(target_arch = "x86_64")]
impl PrivateStaging {
    fn create() -> io::Result<Self> {
        let mut random = [0_u8; 16];
        // SAFETY: random is writable for its full length and system-preferred RNG requires no provider.
        let status = unsafe {
            BCryptGenRandom(
                null_mut(),
                random.as_mut_ptr(),
                random.len() as u32,
                BCRYPT_USE_SYSTEM_PREFERRED_RNG,
            )
        };
        if status < 0 {
            return Err(io::Error::other(
                "BCryptGenRandom failed for PawnIO staging",
            ));
        }
        let token = u128::from_le_bytes(random);
        let path = std::env::temp_dir().join(format!("cclover-mon-pawnio-{token:032x}"));
        create_private_directory(&path)?;
        Ok(Self { path })
    }
}

#[cfg(target_arch = "x86_64")]
impl Drop for PrivateStaging {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[cfg(target_arch = "x86_64")]
fn create_private_directory(path: &Path) -> io::Result<()> {
    let sddl = wide_z(PRIVATE_DIRECTORY_SDDL);
    let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
    // SAFETY: SDDL is nul-terminated and descriptor receives LocalAlloc-owned memory.
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let descriptor = LocalDescriptor(descriptor);
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: 0,
    };
    let path = wide_z(&path.to_string_lossy());
    // SAFETY: path is nul-terminated and security descriptor remains alive for the call.
    if unsafe { CreateDirectoryW(path.as_ptr(), &attributes) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(target_arch = "x86_64")]
struct LocalDescriptor(PSECURITY_DESCRIPTOR);

#[cfg(target_arch = "x86_64")]
impl Drop for LocalDescriptor {
    fn drop(&mut self) {
        // SAFETY: ConvertStringSecurityDescriptor... returns LocalAlloc-owned memory.
        unsafe { LocalFree(self.0.cast()) };
    }
}

#[cfg(target_arch = "x86_64")]
struct OwnedHandle(HANDLE);

#[cfg(target_arch = "x86_64")]
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: this object uniquely owns the handle.
        unsafe { CloseHandle(self.0) };
    }
}

fn devinfo_data() -> SP_DEVINFO_DATA {
    SP_DEVINFO_DATA {
        cbSize: size_of::<SP_DEVINFO_DATA>() as u32,
        ..Default::default()
    }
}

fn wide_z(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(target_arch = "x86_64")]
fn wide_multi_z(value: &str) -> Vec<u16> {
    value
        .encode_utf16()
        .chain(std::iter::once(0))
        .chain(std::iter::once(0))
        .collect()
}

fn multi_sz(value: &[u16]) -> impl Iterator<Item = String> + '_ {
    value
        .split(|unit| *unit == 0)
        .take_while(|part| !part.is_empty())
        .map(String::from_utf16_lossy)
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;

    use super::*;

    struct FakeEnvironment {
        build: Option<u32>,
        versions: RefCell<VecDeque<Option<Version>>>,
        device_present: bool,
        #[cfg(target_arch = "x86_64")]
        helper_result: Result<u32, i32>,
        helper_calls: Cell<u32>,
    }

    impl FakeEnvironment {
        fn new(versions: impl IntoIterator<Item = Option<Version>>) -> Self {
            Self {
                build: Some(minimum_windows_build()),
                versions: RefCell::new(versions.into_iter().collect()),
                device_present: false,
                #[cfg(target_arch = "x86_64")]
                helper_result: Ok(EXIT_READY),
                helper_calls: Cell::new(0),
            }
        }
    }

    impl ProvisionEnvironment for FakeEnvironment {
        fn windows_build(&self) -> Option<u32> {
            self.build
        }

        fn installed_driver_version(&self) -> Option<Version> {
            let mut values = self.versions.borrow_mut();
            if values.len() > 1 {
                values.pop_front().flatten()
            } else {
                values.front().copied().flatten()
            }
        }

        fn pawnio_device_present(&self) -> bool {
            self.device_present
        }

        #[cfg(target_arch = "x86_64")]
        fn launch_elevated_helper(&self) -> io::Result<u32> {
            self.helper_calls.set(self.helper_calls.get() + 1);
            self.helper_result.map_err(io::Error::from_raw_os_error)
        }
    }

    #[test]
    fn versions_compare_numerically_and_allow_newer_compatible_driver() {
        assert!(parse_version("2.2.1.0") > parse_version("2.2.0"));
        assert_eq!(parse_version("2.2.0"), Some(Version([2, 2, 0, 0])));
        assert_eq!(parse_version("2.2.0.0.1"), None);
    }

    #[test]
    fn unsupported_windows_never_attempts_provisioning() {
        let mut environment = FakeEnvironment::new([None]);
        environment.build = Some(minimum_windows_build() - 1);

        assert_eq!(
            prepare_machine_capability_with(&environment),
            MachineStatus::Unsupported
        );
        assert_eq!(environment.helper_calls.get(), 0);
    }

    #[test]
    fn newer_driver_is_reused_without_downgrade_or_elevation() {
        let environment = FakeEnvironment::new([Some(Version([9, 0, 0, 0]))]);

        assert_eq!(
            prepare_machine_capability_with(&environment),
            MachineStatus::Ready
        );
        assert_eq!(environment.helper_calls.get(), 0);
    }

    #[test]
    fn unknown_preexisting_device_is_not_overwritten() {
        let mut environment = FakeEnvironment::new([None]);
        environment.device_present = true;

        assert_eq!(
            prepare_machine_capability_with(&environment),
            MachineStatus::InstallFailed
        );
        assert_eq!(environment.helper_calls.get(), 0);
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn concurrent_install_is_rechecked_after_elevated_helper() {
        let required = required_driver_version();
        let environment = FakeEnvironment::new([None, None, Some(required)]);

        assert_eq!(
            prepare_machine_capability_with(&environment),
            MachineStatus::Ready
        );
        assert_eq!(environment.helper_calls.get(), 1);
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn uac_cancellation_is_a_stable_typed_state() {
        let mut environment = FakeEnvironment::new([None]);
        environment.helper_result = Err(ERROR_CANCELLED as i32);

        assert_eq!(
            prepare_machine_capability_with(&environment),
            MachineStatus::ElevationDeclined
        );
        assert_eq!(environment.helper_calls.get(), 1);
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn reboot_required_is_preserved() {
        let mut environment = FakeEnvironment::new([None]);
        environment.helper_result = Ok(EXIT_REBOOT_REQUIRED);

        assert_eq!(
            prepare_machine_capability_with(&environment),
            MachineStatus::RebootRequired
        );
    }

    #[cfg(not(target_arch = "x86_64"))]
    #[test]
    fn x86_process_does_not_attempt_driver_provisioning() {
        let environment = FakeEnvironment::new([None]);

        assert_eq!(
            prepare_machine_capability_with(&environment),
            MachineStatus::ProvisioningUnsupported
        );
        assert_eq!(environment.helper_calls.get(), 0);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn multi_string_parsing_stops_at_double_nul() {
        let value = wide_multi_z("Root\\PawnIO");
        assert_eq!(multi_sz(&value).collect::<Vec<_>>(), ["Root\\PawnIO"]);
    }
}
