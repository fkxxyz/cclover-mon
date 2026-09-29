// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// https://mozilla.org/MPL/2.0/.
// Copyright (C) LibreHardwareMonitor and Contributors.
// Partial Copyright (C) Michael Möller <mmoeller@openhardwaremonitor.org> and Contributors.
// Fan decode ported for cclover-mon from LibreHardwareMonitor W836XX.cs at reviewed commit
// 677a3a56abde9adff5abdb42db3a7638c9137572.
// cclover-mon intentionally omits LHM's adaptive divisor writes: existing hardware
// divisor state is sufficient to decode RPM and telemetry remains read-only.

use std::io;

use super::access::Access;
use super::chip::Chip;
use super::detect::DeviceDescriptor;

const FAN_BIT_REG: [u8; 5] = [0x47, 0x4B, 0x4C, 0x59, 0x5D];
const FAN_DIV_BIT0: [u8; 5] = [36, 38, 30, 8, 10];
const FAN_DIV_BIT1: [u8; 5] = [37, 39, 31, 9, 11];
const FAN_DIV_BIT2: [u8; 5] = [5, 6, 7, 23, 15];
const FAN_TACHO_BANK: [u8; 5] = [0, 0, 0, 0, 5];
const FAN_TACHO_REG: [u8; 5] = [0x28, 0x29, 0x2A, 0x3F, 0x53];

pub(super) struct Reader {
    descriptor: DeviceDescriptor,
    fan_count: usize,
}

impl Reader {
    pub(super) fn new(
        descriptor: DeviceDescriptor,
        access: &Access<'_>,
    ) -> io::Result<Option<Self>> {
        let fan_count = if matches!(
            descriptor.chip,
            Chip::W83627Ehf | Chip::W83627Dhg | Chip::W83627Dhgp | Chip::W83667Hg | Chip::W83667Hgb
        ) {
            5
        } else {
            3
        };
        let reader = Self {
            descriptor,
            fan_count,
        };
        let vendor = (u16::from(reader.read_byte(access, 0x80, 0x4F)?) << 8)
            | u16::from(reader.read_byte(access, 0, 0x4F)?);
        if vendor != 0x5CA3 {
            return Ok(None);
        }
        Ok(Some(reader))
    }

    pub(super) fn descriptor(&self) -> DeviceDescriptor {
        self.descriptor
    }

    pub(super) fn read_fans(&self, access: &Access<'_>) -> io::Result<Vec<Option<u64>>> {
        let mut bits = 0_u64;
        for register in FAN_BIT_REG {
            bits = (bits << 8) | u64::from(self.read_byte(access, 0, register)?);
        }

        let mut fans = Vec::with_capacity(self.fan_count);
        for index in 0..self.fan_count {
            let count = self.read_byte(access, FAN_TACHO_BANK[index], FAN_TACHO_REG[index])?;
            let divisor_bits = (((bits >> FAN_DIV_BIT2[index]) & 1) << 2)
                | (((bits >> FAN_DIV_BIT1[index]) & 1) << 1)
                | ((bits >> FAN_DIV_BIT0[index]) & 1);
            let divisor = 1_u64 << divisor_bits;
            fans.push(decode_fan(count, divisor));
        }
        Ok(fans)
    }

    fn read_byte(&self, access: &Access<'_>, bank: u8, register: u8) -> io::Result<u8> {
        access.write_port(self.descriptor.base + 0x05, 0x4E)?;
        access.write_port(self.descriptor.base + 0x06, bank)?;
        access.write_port(self.descriptor.base + 0x05, register)?;
        access.read_port(self.descriptor.base + 0x06)
    }
}

fn decode_fan(count: u8, divisor: u64) -> Option<u64> {
    if count == 0 {
        None
    } else if count == 0xFF {
        Some(0)
    } else {
        Some((1_350_000.0 / (f64::from(count) * divisor as f64)).round() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn winbond_decode_uses_existing_divisor_without_mutating_hardware() {
        assert_eq!(decode_fan(0, 1), None);
        assert_eq!(decode_fan(0xFF, 8), Some(0));
        assert_eq!(decode_fan(135, 10), Some(1000));
    }
}
