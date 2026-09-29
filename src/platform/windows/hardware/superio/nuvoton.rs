// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// https://mozilla.org/MPL/2.0/.
// Copyright (C) LibreHardwareMonitor and Contributors.
// Partial Copyright (C) Michael Möller <mmoeller@openhardwaremonitor.org> and Contributors.
// Fan decode ported for cclover-mon from LibreHardwareMonitor Nct677X.cs at reviewed commit
// 677a3a56abde9adff5abdb42db3a7638c9137572.

use std::io;
use std::thread;
use std::time::{Duration, Instant};

use super::access::Access;
use super::chip::Chip;
use super::detect::DeviceDescriptor;

const NUVOTON_VENDOR_ID: u16 = 0x5CA3;
const FAN_COUNT_REGISTERS: [u16; 7] = [0x4B0, 0x4B2, 0x4B4, 0x4B6, 0x4B8, 0x4BA, 0x4CC];
const NCT668X_FAN_RPM_REGISTERS: [u16; 17] = [
    0x140, 0x142, 0x144, 0x146, 0x148, 0x14A, 0x14C, 0x14E, 0x150, 0x152, 0x154, 0x156, 0x158,
    0x15A, 0x15C, 0x15E, 0x852,
];

pub(super) struct Reader {
    descriptor: DeviceDescriptor,
    kind: Kind,
}

enum Kind {
    DirectRpm { registers: Vec<u16>, minimum: u16 },
    Count13 { registers: Vec<u16> },
    Nct668x,
}

impl Reader {
    pub(super) fn new(
        descriptor: DeviceDescriptor,
        access: &Access<'_>,
    ) -> io::Result<Option<Self>> {
        let kind = match descriptor.chip {
            Chip::Nct610Xd => Kind::DirectRpm {
                registers: (0..3).map(|i| 0x030 + (i << 1)).collect(),
                minimum: (1_350_000 / 0x1FFF) as u16,
            },
            Chip::Nct6771F => Kind::DirectRpm {
                registers: (0..4).map(|i| 0x656 + (i << 1)).collect(),
                minimum: (1_350_000 / 0xFFFF) as u16,
            },
            Chip::Nct6776F => Kind::DirectRpm {
                registers: (0..5).map(|i| 0x656 + (i << 1)).collect(),
                minimum: (1_350_000 / 0x1FFF) as u16,
            },
            Chip::Nct6779D => Kind::Count13 {
                registers: FAN_COUNT_REGISTERS[..5].to_vec(),
            },
            Chip::Nct6797D | Chip::Nct6798D | Chip::Nct6701D => Kind::Count13 {
                registers: FAN_COUNT_REGISTERS.to_vec(),
            },
            Chip::Nct6791D
            | Chip::Nct6792D
            | Chip::Nct6792Da
            | Chip::Nct6793D
            | Chip::Nct6795D
            | Chip::Nct6796D => Kind::Count13 {
                registers: FAN_COUNT_REGISTERS[..6].to_vec(),
            },
            Chip::Nct6683D | Chip::Nct6686D | Chip::Nct6687D => Kind::Nct668x,
            _ => return Ok(None),
        };

        let reader = Self { descriptor, kind };
        if !reader.is_nuvoton_vendor(access)? {
            return Ok(None);
        }
        if matches!(reader.kind, Kind::Nct668x) {
            // LHM enables the EC hardware-monitor register window before reading sensors.
            // This is read-protocol setup, not fan-control policy.
            let value = reader.read_byte(access, 0x180)?;
            if value & 0x80 == 0 {
                reader.write_byte(access, 0x180, value | 0x80)?;
            }
        }
        Ok(Some(reader))
    }

    pub(super) fn descriptor(&self) -> DeviceDescriptor {
        self.descriptor
    }

    pub(super) fn read_fans(&self, access: &Access<'_>) -> io::Result<Vec<Option<u64>>> {
        match &self.kind {
            Kind::DirectRpm { registers, minimum } => registers
                .iter()
                .map(|&register| {
                    let high = self.read_byte(access, register)?;
                    let low = self.read_byte(access, register + 1)?;
                    let rpm = (u16::from(high) << 8) | u16::from(low);
                    Ok(Some(if rpm > *minimum { u64::from(rpm) } else { 0 }))
                })
                .collect(),
            Kind::Count13 { registers } => registers
                .iter()
                .map(|&register| self.read_13_bit_fan(access, register))
                .collect(),
            Kind::Nct668x => NCT668X_FAN_RPM_REGISTERS
                .iter()
                .map(|&register| {
                    let high = self.read_byte(access, register)?;
                    let low = self.read_byte(access, register + 1)?;
                    if register == 0x852 {
                        Ok(decode_13_bit(
                            low,
                            high,
                            self.descriptor.chip == Chip::Nct6687D,
                        ))
                    } else {
                        Ok(Some(u64::from((u16::from(high) << 8) | u16::from(low))))
                    }
                })
                .collect(),
        }
    }

    fn read_13_bit_fan(&self, access: &Access<'_>, register: u16) -> io::Result<Option<u64>> {
        let high = self.read_byte(access, register)?;
        let low = self.read_byte(access, register + 1)?;
        Ok(decode_13_bit(low, high, false))
    }

    fn is_nuvoton_vendor(&self, access: &Access<'_>) -> io::Result<bool> {
        if matches!(
            self.descriptor.chip,
            Chip::Nct6683D | Chip::Nct6686D | Chip::Nct6687D | Chip::Nct6701D
        ) {
            return Ok(true);
        }
        let (high_register, low_register) = if self.descriptor.chip == Chip::Nct610Xd {
            (0x80FE, 0x00FE)
        } else {
            (0x804F, 0x004F)
        };
        let high = self.read_byte(access, high_register)?;
        let low = self.read_byte(access, low_register)?;
        Ok((u16::from(high) << 8) | u16::from(low) == NUVOTON_VENDOR_ID)
    }

    fn read_byte(&self, access: &Access<'_>, address: u16) -> io::Result<u8> {
        if !matches!(
            self.descriptor.chip,
            Chip::Nct6683D | Chip::Nct6686D | Chip::Nct6687D
        ) {
            let bank = (address >> 8) as u8;
            let register = address as u8;
            access.write_port(self.descriptor.base + 0x05, 0x4E)?;
            access.write_port(self.descriptor.base + 0x06, bank)?;
            access.write_port(self.descriptor.base + 0x05, register)?;
            return access.read_port(self.descriptor.base + 0x06);
        }

        self.wait_for_ec_space(access)?;
        let page = (address >> 8) as u8;
        let index = address as u8;
        access.write_port(self.descriptor.base + 0x04, page)?;
        access.write_port(self.descriptor.base + 0x05, index)?;
        let result = access.read_port(self.descriptor.base + 0x06)?;
        access.write_port(self.descriptor.base + 0x04, 0xFF)?;
        Ok(result)
    }

    fn write_byte(&self, access: &Access<'_>, address: u16, value: u8) -> io::Result<()> {
        if !matches!(
            self.descriptor.chip,
            Chip::Nct6683D | Chip::Nct6686D | Chip::Nct6687D
        ) {
            let bank = (address >> 8) as u8;
            let register = address as u8;
            access.write_port(self.descriptor.base + 0x05, 0x4E)?;
            access.write_port(self.descriptor.base + 0x06, bank)?;
            access.write_port(self.descriptor.base + 0x05, register)?;
            return access.write_port(self.descriptor.base + 0x06, value);
        }

        self.wait_for_ec_space(access)?;
        access.write_port(self.descriptor.base + 0x04, (address >> 8) as u8)?;
        access.write_port(self.descriptor.base + 0x05, address as u8)?;
        access.write_port(self.descriptor.base + 0x06, value)?;
        access.write_port(self.descriptor.base + 0x04, 0xFF)
    }

    fn wait_for_ec_space(&self, access: &Access<'_>) -> io::Result<()> {
        let start = Instant::now();
        loop {
            let current = access.read_port(self.descriptor.base + 0x04)?;
            if current == 0xFF {
                return Ok(());
            }
            if start.elapsed() >= Duration::from_millis(500) {
                // Match LHM: force page-select ownership after bounded wait.
                access.write_port(self.descriptor.base + 0x04, 0xFF)?;
                return Ok(());
            }
            thread::sleep(Duration::from_millis(1));
        }
    }
}

fn decode_13_bit(low: u8, high: u8, nct6687_zero_encoding: bool) -> Option<u64> {
    if nct6687_zero_encoding && high == 0xFF && low == 0xF8 {
        return Some(0);
    }
    let count = (u16::from(high) << 5) | u16::from(low & 0x1F);
    if count >= 0x1FFF {
        Some(0)
    } else if count < 0x15 {
        None
    } else {
        Some((1_350_000.0 / f64::from(count)).round() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nuvoton_13_bit_decode_matches_lhm_boundaries() {
        assert_eq!(decode_13_bit(0, 0, false), None);
        assert_eq!(decode_13_bit(0x1F, 0xFF, false), Some(0));
        assert_eq!(decode_13_bit(0x15, 0, false), Some(64_286));
        assert_eq!(decode_13_bit(0xF8, 0xFF, true), Some(0));
    }
}
