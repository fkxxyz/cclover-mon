use super::*;

pub(super) fn required_driver_version() -> Version {
    parse_version(env!("CCLOVER_PAWNIO_DRIVER_VERSION")).expect("prepared PawnIO version is valid")
}

pub(super) fn minimum_windows_build() -> u32 {
    env!("CCLOVER_PAWNIO_MIN_WINDOWS_BUILD")
        .parse()
        .expect("prepared PawnIO minimum Windows build is valid")
}

pub(super) fn windows_build() -> Option<u32> {
    let mut version = OSVERSIONINFOW {
        dwOSVersionInfoSize: size_of::<OSVERSIONINFOW>() as u32,
        ..Default::default()
    };
    // SAFETY: version points to a correctly sized writable OSVERSIONINFOW.
    let status = unsafe { RtlGetVersion(&mut version) };
    (status >= 0).then_some(version.dwBuildNumber)
}

pub(super) fn installed_driver_version() -> Option<Version> {
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

pub(super) fn pawnio_device_present() -> bool {
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

pub(super) fn parse_version(value: &str) -> Option<Version> {
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
