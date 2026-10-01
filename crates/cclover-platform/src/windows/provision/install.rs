use super::*;

#[cfg(target_arch = "x86_64")]
pub(super) fn launch_elevated_helper() -> io::Result<u32> {
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
pub(super) fn elevated_install() -> io::Result<bool> {
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

pub(super) struct DeviceInfoSet(pub(super) HDEVINFO);

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

pub(super) fn devinfo_data() -> SP_DEVINFO_DATA {
    SP_DEVINFO_DATA {
        cbSize: size_of::<SP_DEVINFO_DATA>() as u32,
        ..Default::default()
    }
}

pub(super) fn wide_z(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(target_arch = "x86_64")]
pub(super) fn wide_multi_z(value: &str) -> Vec<u16> {
    value
        .encode_utf16()
        .chain(std::iter::once(0))
        .chain(std::iter::once(0))
        .collect()
}

pub(super) fn multi_sz(value: &[u16]) -> impl Iterator<Item = String> + '_ {
    value
        .split(|unit| *unit == 0)
        .take_while(|part| !part.is_empty())
        .map(String::from_utf16_lossy)
}
