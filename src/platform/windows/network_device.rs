#![allow(unsafe_code)]

use std::collections::HashMap;
use std::io;
use std::mem::{size_of, size_of_val, zeroed};
use std::ptr::{null, null_mut};

use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    DICS_FLAG_GLOBAL, DIGCF_PRESENT, DIREG_DRV, GUID_DEVCLASS_NET, HDEVINFO, SP_DEVINFO_DATA,
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo, SetupDiGetClassDevsW,
    SetupDiGetDevicePropertyW, SetupDiOpenDevRegKey,
};
use windows_sys::Win32::Devices::Properties::{
    DEVPKEY_Device_BusTypeGuid, DEVPKEY_Device_EnumeratorName, DEVPROP_TYPE_GUID,
    DEVPROP_TYPE_STRING,
};
use windows_sys::Win32::Foundation::{
    ERROR_NO_MORE_ITEMS, ERROR_SUCCESS, GetLastError, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::System::Registry::{
    HKEY, KEY_READ, RRF_RT_REG_SZ, RegCloseKey, RegGetValueW,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NetworkDeviceClass {
    HardwareBacked,
    Software,
    Unknown,
}

pub(super) fn classify(candidate_ids: &[String]) -> io::Result<Vec<NetworkDeviceClass>> {
    if candidate_ids.is_empty() {
        return Ok(Vec::new());
    }

    let set = DeviceInfoSet::network_devices()?;
    let mut by_interface = HashMap::new();
    let mut index = 0;
    loop {
        let mut info = devinfo_data();
        // SAFETY: set is live and info is a correctly initialized writable SP_DEVINFO_DATA.
        if unsafe { SetupDiEnumDeviceInfo(set.0, index, &mut info) } == 0 {
            // SAFETY: immediately reads the thread-local error from the failed API call.
            let error = unsafe { GetLastError() };
            if error == ERROR_NO_MORE_ITEMS {
                break;
            }
            return Err(io::Error::from_raw_os_error(error as i32));
        }
        index += 1;

        let Some(interface_id) = netcfg_instance_id(set.0, &info) else {
            continue;
        };
        by_interface.insert(normalize_guid(&interface_id), classify_device(set.0, &info));
    }

    Ok(candidate_ids
        .iter()
        .map(|id| {
            by_interface
                .get(&normalize_guid(id))
                .copied()
                .unwrap_or(NetworkDeviceClass::Unknown)
        })
        .collect())
}

fn classify_device(set: HDEVINFO, info: &SP_DEVINFO_DATA) -> NetworkDeviceClass {
    let enumerator = device_string_property(set, info, &DEVPKEY_Device_EnumeratorName);
    classify_properties(enumerator.as_deref(), has_bus_type(set, info))
}

fn classify_properties(enumerator: Option<&str>, has_bus_type: bool) -> NetworkDeviceClass {
    if enumerator.is_some_and(|value| {
        value.eq_ignore_ascii_case("ROOT") || value.eq_ignore_ascii_case("SWD")
    }) {
        NetworkDeviceClass::Software
    } else if has_bus_type {
        NetworkDeviceClass::HardwareBacked
    } else {
        NetworkDeviceClass::Unknown
    }
}

fn has_bus_type(set: HDEVINFO, info: &SP_DEVINFO_DATA) -> bool {
    let mut property_type = 0_u32;
    // SAFETY: GUID is a plain C data structure for an output buffer.
    let mut bus_type: windows_sys::core::GUID = unsafe { zeroed() };
    let mut required = 0_u32;
    // SAFETY: bus_type is writable GUID storage; set/info are live for this call.
    unsafe {
        SetupDiGetDevicePropertyW(
            set,
            info,
            &DEVPKEY_Device_BusTypeGuid,
            &mut property_type,
            (&mut bus_type as *mut windows_sys::core::GUID).cast(),
            size_of::<windows_sys::core::GUID>() as u32,
            &mut required,
            0,
        ) != 0
            && property_type == DEVPROP_TYPE_GUID
    }
}

fn device_string_property(
    set: HDEVINFO,
    info: &SP_DEVINFO_DATA,
    key: &windows_sys::Win32::Foundation::DEVPROPKEY,
) -> Option<String> {
    let mut property_type = 0_u32;
    let mut buffer = [0_u16; 128];
    let mut required = 0_u32;
    // SAFETY: buffer is writable for its advertised byte length; set/info/key are valid.
    if unsafe {
        SetupDiGetDevicePropertyW(
            set,
            info,
            key,
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
    Some(utf16_z(&buffer))
}

fn netcfg_instance_id(set: HDEVINFO, info: &SP_DEVINFO_DATA) -> Option<String> {
    // SAFETY: set/info are live; requested access is read-only to the global driver key.
    let key = unsafe { SetupDiOpenDevRegKey(set, info, DICS_FLAG_GLOBAL, 0, DIREG_DRV, KEY_READ) };
    if key == INVALID_HANDLE_VALUE {
        return None;
    }
    let key = RegistryKey(key);
    let value_name = wide_z("NetCfgInstanceId");
    let mut buffer = [0_u16; 64];
    let mut bytes = size_of_val(&buffer) as u32;
    // SAFETY: key is live; value_name is nul-terminated; buffer and byte count are writable.
    let status = unsafe {
        RegGetValueW(
            key.0,
            null(),
            value_name.as_ptr(),
            RRF_RT_REG_SZ,
            null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut bytes,
        )
    };
    (status == ERROR_SUCCESS).then(|| utf16_z(&buffer))
}

fn normalize_guid(value: &str) -> String {
    value.trim().trim_matches(['{', '}']).to_ascii_lowercase()
}

fn utf16_z(value: &[u16]) -> String {
    let len = value
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(value.len());
    String::from_utf16_lossy(&value[..len])
}

fn wide_z(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn devinfo_data() -> SP_DEVINFO_DATA {
    SP_DEVINFO_DATA {
        cbSize: size_of::<SP_DEVINFO_DATA>() as u32,
        ..Default::default()
    }
}

struct DeviceInfoSet(HDEVINFO);

impl DeviceInfoSet {
    fn network_devices() -> io::Result<Self> {
        // SAFETY: class GUID is valid; no enumerator/window is supplied; returned set is owned below.
        let set =
            unsafe { SetupDiGetClassDevsW(&GUID_DEVCLASS_NET, null(), null_mut(), DIGCF_PRESENT) };
        if set == -1_isize {
            Err(io::Error::last_os_error())
        } else {
            Ok(Self(set))
        }
    }
}

impl Drop for DeviceInfoSet {
    fn drop(&mut self) {
        // SAFETY: self.0 is a successful SetupDiGetClassDevsW result and is destroyed once.
        unsafe { SetupDiDestroyDeviceInfoList(self.0) };
    }
}

struct RegistryKey(HKEY);

impl Drop for RegistryKey {
    fn drop(&mut self) {
        // SAFETY: self.0 is a successful SetupDiOpenDevRegKey result and is closed once.
        unsafe { RegCloseKey(self.0) };
    }
}

#[cfg(test)]
mod tests {
    use super::{NetworkDeviceClass, classify_properties, normalize_guid};

    #[test]
    fn root_and_software_enumerators_are_not_hardware_backed() {
        assert_eq!(
            classify_properties(Some("ROOT"), false),
            NetworkDeviceClass::Software
        );
        assert_eq!(
            classify_properties(Some("SWD"), true),
            NetworkDeviceClass::Software
        );
    }

    #[test]
    fn bus_backed_devices_are_hardware_candidates() {
        assert_eq!(
            classify_properties(Some("PCI"), true),
            NetworkDeviceClass::HardwareBacked
        );
        assert_eq!(
            classify_properties(Some("USB"), true),
            NetworkDeviceClass::HardwareBacked
        );
    }

    #[test]
    fn missing_provenance_is_unknown_not_software() {
        assert_eq!(
            classify_properties(Some("PCI"), false),
            NetworkDeviceClass::Unknown
        );
        assert_eq!(
            classify_properties(None, false),
            NetworkDeviceClass::Unknown
        );
    }

    #[test]
    fn interface_guid_normalization_ignores_braces_and_case() {
        assert_eq!(normalize_guid("{ABC-123}"), "abc-123");
    }
}
