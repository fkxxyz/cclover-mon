// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// https://mozilla.org/MPL/2.0/.
// Copyright (C) LibreHardwareMonitor and Contributors.
// Partial Copyright (C) Michael Möller <mmoeller@openhardwaremonitor.org> and Contributors.
// Ported for cclover-mon from LibreHardwareMonitor LpcIO.cs/LpcPort.cs at reviewed commit
// 677a3a56abde9adff5abdb42db3a7638c9137572.

use std::io;
use std::thread;
use std::time::Duration;

use super::super::board;
use super::access::Access;
use super::chip::Chip;

const CHIP_ID_REGISTER: u8 = 0x20;
const CHIP_REVISION_REGISTER: u8 = 0x21;
const BASE_ADDRESS_REGISTER: u8 = 0x60;
const FINTEK_VENDOR_ID_REGISTER: u8 = 0x23;
const FINTEK_VENDOR_ID: u16 = 0x1934;
const WINBOND_NUVOTON_HARDWARE_MONITOR_LDN: u8 = 0x0B;
const FINTEK_HARDWARE_MONITOR_LDN: u8 = 0x04;
const F71858_HARDWARE_MONITOR_LDN: u8 = 0x02;
const IT87_ENVIRONMENT_CONTROLLER_LDN: u8 = 0x04;
const IT87_CHIP_VERSION_REGISTER: u8 = 0x22;

#[derive(Clone, Copy, Debug)]
pub(super) struct DeviceDescriptor {
    pub slot: u8,
    pub chip: Chip,
    pub base: u16,
    pub ite_version: u8,
}

pub(super) struct Discovery {
    pub devices: Vec<DeviceDescriptor>,
    pub details: Vec<String>,
}

pub(super) fn discover(access: &Access<'_>, board: Option<&board::Info>) -> io::Result<Discovery> {
    let mut devices = Vec::new();
    let mut details = Vec::new();
    for slot in 0..=1_u8 {
        access.select_slot(u64::from(slot))?;
        let (winbond_device, id, revision) = detect_winbond_nuvoton_fintek(access, slot, board)?;
        if let Some(device) = winbond_device {
            devices.push(device);
            continue;
        }
        let (ite_device, ite_id) = detect_ite(access, slot)?;
        if let Some(device) = ite_device {
            devices.push(device);
        } else {
            details.push(format!(
                "slot={slot} ports=0x{:02X}/0x{:02X} winbond/nuvoton/fintek=0x{id:02X}:0x{revision:02X} ite=0x{ite_id:04X}",
                register_port(slot),
                register_port(slot) + 1,
            ));
        }
    }
    Ok(Discovery { devices, details })
}

fn detect_winbond_nuvoton_fintek(
    access: &Access<'_>,
    slot: u8,
    board: Option<&board::Info>,
) -> io::Result<(Option<DeviceDescriptor>, u8, u8)> {
    let register_port = register_port(slot);
    access.write_port(register_port, 0x87)?;
    access.write_port(register_port, 0x87)?;

    let id = access.read_config_byte(CHIP_ID_REGISTER)?;
    let revision = access.read_config_byte(CHIP_REVISION_REGISTER)?;
    let Some((chip, ldn)) = winbond_nuvoton_fintek_chip(id, revision, board) else {
        access.write_port(register_port, 0xAA)?;
        return Ok((None, id, revision));
    };

    access.find_bars()?;
    access.write_config_byte(0x07, ldn)?;
    if chip == Chip::Nct6701D && access.read_config_byte(0x30)? == 0 {
        access.write_config_byte(0x30, 0x01)?;
    }
    let mut address = stable_config_word(access, BASE_ADDRESS_REGISTER)?;
    if chip == Chip::Nct6701D && invalid_runtime_base(address) {
        address = stable_config_word(access, 0x64)?;
    }
    let vendor_id = access.read_config_word(FINTEK_VENDOR_ID_REGISTER)?;

    if matches!(
        chip,
        Chip::Nct6791D
            | Chip::Nct6792D
            | Chip::Nct6792Da
            | Chip::Nct6793D
            | Chip::Nct6795D
            | Chip::Nct6796D
            | Chip::Nct6796Dr
            | Chip::Nct6796Ds
            | Chip::Nct6797D
            | Chip::Nct6798D
            | Chip::Nct6799D
            | Chip::Nct5585D
            | Chip::Nct6701D
    ) {
        let options = access.read_config_byte(0x28)?;
        if options & 0x10 != 0 {
            access.write_config_byte(0x28, options & !0x10)?;
        }
    }

    access.write_port(register_port, 0xAA)?;

    if chip.is_fintek() && vendor_id != FINTEK_VENDOR_ID {
        return Ok((None, id, revision));
    }
    let mut base = address;
    if chip.is_fintek() && base & 0x07 == 0x05 {
        base &= 0xFFF8;
    }
    if invalid_runtime_base(base) {
        return Ok((None, id, revision));
    }

    Ok((
        Some(DeviceDescriptor {
            slot,
            chip,
            base,
            ite_version: 0,
        }),
        id,
        revision,
    ))
}

fn detect_ite(access: &Access<'_>, slot: u8) -> io::Result<(Option<DeviceDescriptor>, u16)> {
    let register_port = register_port(slot);
    let value_port = register_port + 1;

    let mut chip_id = if slot == 1 {
        access.read_config_word(CHIP_ID_REGISTER)?
    } else {
        0xFFFF
    };
    if slot == 0 || chip_id == 0xFFFF {
        access.write_port(register_port, 0x87)?;
        access.write_port(register_port, 0x01)?;
        access.write_port(register_port, 0x55)?;
        access.write_port(register_port, if slot == 1 { 0xAA } else { 0x55 })?;
        chip_id = access.read_config_word(CHIP_ID_REGISTER)?;
    }

    let Some(chip) = ite_chip(chip_id) else {
        if slot == 0 {
            access.write_port(register_port, 0x02)?;
            access.write_port(value_port, 0x02)?;
        }
        return Ok((None, chip_id));
    };

    access.find_bars()?;
    access.write_config_byte(0x07, IT87_ENVIRONMENT_CONTROLLER_LDN)?;
    let base = stable_config_word(access, BASE_ADDRESS_REGISTER)?;
    let version = access.read_config_byte(IT87_CHIP_VERSION_REGISTER)? & 0x0F;

    if slot == 0 {
        access.write_port(register_port, 0x02)?;
        access.write_port(value_port, 0x02)?;
    }

    if invalid_runtime_base(base) {
        return Ok((None, chip_id));
    }

    Ok((
        Some(DeviceDescriptor {
            slot,
            chip,
            base,
            ite_version: version,
        }),
        chip_id,
    ))
}

fn stable_config_word(access: &Access<'_>, register: u8) -> io::Result<u16> {
    let first = access.read_config_word(register)?;
    thread::sleep(Duration::from_millis(1));
    let second = access.read_config_word(register)?;
    if first != second {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Super-I/O base address verification failed",
        ));
    }
    Ok(first)
}

fn invalid_runtime_base(address: u16) -> bool {
    address < 0x100 || (address & 0xF007) != 0
}

fn register_port(slot: u8) -> u16 {
    if slot == 0 { 0x2E } else { 0x4E }
}

fn winbond_nuvoton_fintek_chip(
    id: u8,
    revision: u8,
    board: Option<&board::Info>,
) -> Option<(Chip, u8)> {
    let hwmon = WINBOND_NUVOTON_HARDWARE_MONITOR_LDN;
    let fintek = FINTEK_HARDWARE_MONITOR_LDN;
    let value = match (id, revision) {
        (0x05, 0x07) => (Chip::F71858, F71858_HARDWARE_MONITOR_LDN),
        (0x05, 0x41) => (Chip::F71882, fintek),
        (0x06, 0x01) => (Chip::F71862, fintek),
        (0x07, 0x23) => (Chip::F71889F, fintek),
        (0x08, 0x14) => (Chip::F71869, fintek),
        (0x09, 0x01) => (Chip::F71808E, fintek),
        (0x09, 0x09) => (Chip::F71889ED, fintek),
        (0x10, 0x05) => (Chip::F71889AD, fintek),
        (0x10, 0x07) => (Chip::F71869A, fintek),
        (0x11, 0x06) => (Chip::F71878AD, fintek),
        (0x52, 0x17 | 0x3A | 0x41) => (Chip::W83627Hf, hwmon),
        (0x82, rev) if rev & 0xF0 == 0x80 => (Chip::W83627Thf, hwmon),
        (0x85, 0x41) => (Chip::W83687Thf, hwmon),
        (0x88, rev) if matches!(rev & 0xF0, 0x50 | 0x60) => (Chip::W83627Ehf, hwmon),
        (0xA0, rev) if rev & 0xF0 == 0x20 => (Chip::W83627Dhg, hwmon),
        (0xA5, rev) if rev & 0xF0 == 0x10 => (Chip::W83667Hg, hwmon),
        (0xB0, rev) if rev & 0xF0 == 0x70 => (Chip::W83627Dhgp, hwmon),
        (0xB3, rev) if rev & 0xF0 == 0x50 => (Chip::W83667Hgb, hwmon),
        (0xB4, rev) if rev & 0xF0 == 0x70 => (Chip::Nct6771F, hwmon),
        (0xC3, rev) if rev & 0xF0 == 0x30 => (Chip::Nct6776F, hwmon),
        (0xC4, rev) if rev & 0xF0 == 0x50 => (Chip::Nct610Xd, hwmon),
        (0xC5, rev) if rev & 0xF0 == 0x60 => (Chip::Nct6779D, hwmon),
        (0xC7, 0x32) => (Chip::Nct6683D, hwmon),
        (0xC8, 0x03) => (Chip::Nct6791D, hwmon),
        (0xC9, 0x11) => (Chip::Nct6792D, hwmon),
        (0xC9, 0x13) => (Chip::Nct6792Da, hwmon),
        (0xD1, 0x21) => (Chip::Nct6793D, hwmon),
        (0xD3, 0x52) => (Chip::Nct6795D, hwmon),
        (0xD4, 0x23) => (Chip::Nct6796D, hwmon),
        (0xD4, 0x2A) => {
            let board = board?;
            if board.is_asrock() && board.product_is("X870E_NOVA_WIFI") {
                (Chip::Nct5585D, hwmon)
            } else {
                (Chip::Nct6796Dr, hwmon)
            }
        }
        (0xD4, 0x51) => (Chip::Nct6797D, hwmon),
        (0xD4, 0x2B) => (Chip::Nct6798D, hwmon),
        (0xD4, 0x40 | 0x41) => (Chip::Nct6686D, hwmon),
        (0xD5, 0x92) => {
            let board = board?;
            if board.is_msi()
                && ["B840", "B850", "B860", "X870", "Z890"]
                    .iter()
                    .any(|family| board.product_contains(family))
            {
                (Chip::Nct6687Dr, hwmon)
            } else {
                (Chip::Nct6687D, hwmon)
            }
        }
        (0xD8, 0x02) => {
            let board = board?;
            if board.product_is("X870E_NOVA_WIFI") || board.product_is("B650M_HDV_M_2") {
                (Chip::Nct6796Ds, hwmon)
            } else {
                (Chip::Nct6799D, hwmon)
            }
        }
        (0xD8, 0x06) => (Chip::Nct6701D, hwmon),
        _ => return None,
    };
    Some(value)
}

fn ite_chip(id: u16) -> Option<Chip> {
    Some(match id {
        0x8613 => Chip::It8613E,
        0x8620 => Chip::It8620E,
        0x8625 => Chip::It8625E,
        0x8628 => Chip::It8628E,
        0x8631 => Chip::It8631E,
        0x8638 => Chip::It8638E,
        0x8655 => Chip::It8655E,
        0x8665 => Chip::It8665E,
        0x8686 => Chip::It8686E,
        0x8688 => Chip::It8688E,
        0x8689 => Chip::It8689E,
        0x8696 => Chip::It8696E,
        0x8705 => Chip::It8705F,
        0x8712 => Chip::It8712F,
        0x8716 => Chip::It8716F,
        0x8718 => Chip::It8718F,
        0x8720 => Chip::It8720F,
        0x8721 => Chip::It8721F,
        0x8726 => Chip::It8726F,
        0x8728 => Chip::It8728F,
        0x8771 => Chip::It8771E,
        0x8772 => Chip::It8772E,
        0x8790 => Chip::It8790E,
        0x8733 => Chip::It8792E,
        0x8695 => Chip::It87952E,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ambiguous_board_dependent_nuvoton_ids_are_not_guessed() {
        assert_eq!(winbond_nuvoton_fintek_chip(0xD4, 0x2A, None), None);
        assert_eq!(winbond_nuvoton_fintek_chip(0xD8, 0x02, None), None);
    }

    #[test]
    fn common_chip_ids_match_reviewed_lhm_table() {
        assert_eq!(ite_chip(0x8688), Some(Chip::It8688E));
        assert_eq!(
            winbond_nuvoton_fintek_chip(0xD4, 0x2B, None).map(|x| x.0),
            Some(Chip::Nct6798D)
        );
        assert_eq!(
            winbond_nuvoton_fintek_chip(0x10, 0x05, None).map(|x| x.0),
            Some(Chip::F71889AD)
        );
    }
}
