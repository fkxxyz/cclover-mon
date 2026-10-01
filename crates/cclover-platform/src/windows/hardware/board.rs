#![allow(unsafe_code)]

use std::io;
use std::ptr::null_mut;

use windows_sys::Win32::System::SystemInformation::{GetSystemFirmwareTable, RSMB};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct Info {
    pub manufacturer: String,
    pub product: String,
}

impl Info {
    pub(super) fn is_asus(&self) -> bool {
        let manufacturer = normalize(&self.manufacturer);
        manufacturer.starts_with("asus") || manufacturer.starts_with("asustek")
    }

    pub(super) fn is_msi(&self) -> bool {
        let manufacturer = normalize(&self.manufacturer);
        manufacturer == "msi" || manufacturer.contains("microstarinternational")
    }

    pub(super) fn is_asrock(&self) -> bool {
        normalize(&self.manufacturer).contains("asrock")
    }

    pub(super) fn product_is(&self, value: &str) -> bool {
        normalize(&self.product) == normalize(value)
    }

    pub(super) fn product_contains(&self, value: &str) -> bool {
        normalize(&self.product).contains(&normalize(value))
    }

    pub(super) fn product_key(&self) -> String {
        normalize(&self.product)
    }
}

pub(super) fn detect() -> io::Result<Info> {
    // SAFETY: null sizing call is documented by GetSystemFirmwareTable.
    let required = unsafe { GetSystemFirmwareTable(RSMB, 0, null_mut(), 0) };
    if required == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut buffer = vec![0_u8; required as usize];
    // SAFETY: buffer is writable for required bytes.
    let returned = unsafe { GetSystemFirmwareTable(RSMB, 0, buffer.as_mut_ptr(), required) };
    if returned == 0 {
        return Err(io::Error::last_os_error());
    }
    if returned as usize > buffer.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "SMBIOS firmware table grew during read",
        ));
    }
    buffer.truncate(returned as usize);
    parse_raw_smbios(&buffer)
}

fn parse_raw_smbios(raw: &[u8]) -> io::Result<Info> {
    // RawSMBIOSData: four version bytes followed by a little-endian table length.
    if raw.len() < 8 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "truncated RawSMBIOSData header",
        ));
    }
    let table_len = u32::from_le_bytes(raw[4..8].try_into().unwrap()) as usize;
    let end = 8_usize
        .checked_add(table_len)
        .filter(|&end| end <= raw.len())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid SMBIOS table length"))?;

    let mut board = None;
    let mut system = None;
    let mut offset = 8_usize;
    while offset + 4 <= end {
        let kind = raw[offset];
        let formatted_len = usize::from(raw[offset + 1]);
        if formatted_len < 4 || offset + formatted_len > end {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid SMBIOS structure length",
            ));
        }
        let strings_start = offset + formatted_len;
        let strings_end = find_double_nul(raw, strings_start, end).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "unterminated SMBIOS string table",
            )
        })?;
        if formatted_len >= 6 && matches!(kind, 1 | 2) {
            let manufacturer = smbios_string(raw, strings_start, strings_end, raw[offset + 4]);
            let product = smbios_string(raw, strings_start, strings_end, raw[offset + 5]);
            let candidate = Info {
                manufacturer,
                product,
            };
            if kind == 2 && (!candidate.manufacturer.is_empty() || !candidate.product.is_empty()) {
                board = Some(candidate);
            } else if kind == 1
                && (!candidate.manufacturer.is_empty() || !candidate.product.is_empty())
            {
                system = Some(candidate);
            }
        }
        offset = strings_end + 2;
        if kind == 127 {
            break;
        }
    }
    board.or(system).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "SMBIOS contains no system/baseboard identity",
        )
    })
}

fn find_double_nul(raw: &[u8], start: usize, end: usize) -> Option<usize> {
    if start + 1 > end {
        return None;
    }
    (start..end.saturating_sub(1)).find(|&index| raw[index] == 0 && raw[index + 1] == 0)
}

fn smbios_string(raw: &[u8], start: usize, end: usize, index: u8) -> String {
    if index == 0 {
        return String::new();
    }
    let mut current = 1_u8;
    let mut cursor = start;
    while cursor < end {
        let next = raw[cursor..end]
            .iter()
            .position(|&byte| byte == 0)
            .map(|relative| cursor + relative)
            .unwrap_or(end);
        if current == index {
            return String::from_utf8_lossy(&raw[cursor..next])
                .trim()
                .to_owned();
        }
        current = current.saturating_add(1);
        cursor = next.saturating_add(1);
    }
    String::new()
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn structure(kind: u8, manufacturer: u8, product: u8, strings: &[&str]) -> Vec<u8> {
        let mut result = vec![kind, 6, 0, 0, manufacturer, product];
        for value in strings {
            result.extend_from_slice(value.as_bytes());
            result.push(0);
        }
        result.push(0);
        result
    }

    fn raw_table(structures: &[Vec<u8>]) -> Vec<u8> {
        let table: Vec<u8> = structures.iter().flatten().copied().collect();
        let mut raw = vec![0, 3, 7, 0];
        raw.extend_from_slice(&(table.len() as u32).to_le_bytes());
        raw.extend_from_slice(&table);
        raw
    }

    #[test]
    fn baseboard_identity_wins_over_system_identity() {
        let raw = raw_table(&[
            structure(1, 1, 2, &["System Vendor", "System Product"]),
            structure(2, 1, 2, &["ASRock", "X870E Nova WiFi"]),
            vec![127, 4, 0, 0, 0, 0],
        ]);
        let info = parse_raw_smbios(&raw).unwrap();
        assert_eq!(info.manufacturer, "ASRock");
        assert_eq!(info.product, "X870E Nova WiFi");
        assert!(info.is_asrock());
        assert!(info.product_is("X870E_NOVA_WIFI"));
    }

    #[test]
    fn msi_alias_and_product_matching_are_format_insensitive() {
        let info = Info {
            manufacturer: "Micro-Star International Co., Ltd.".to_owned(),
            product: "MAG X870 TOMAHAWK WIFI".to_owned(),
        };
        assert!(info.is_msi());
        assert!(info.product_contains("X870"));
    }
}
