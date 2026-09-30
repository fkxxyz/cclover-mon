// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// https://mozilla.org/MPL/2.0/.
// Copyright (C) LibreHardwareMonitor and Contributors.
// Partial Copyright (C) Michael Möller <mmoeller@openhardwaremonitor.org> and Contributors.
// Temperature/fan decode ported for cclover-mon from LibreHardwareMonitor F718XX.cs at reviewed commit
// 677a3a56abde9adff5abdb42db3a7638c9137572.

use std::io;

use super::access::Access;
use super::chip::Chip;
use super::detect::DeviceDescriptor;

pub(super) struct Reader {
    descriptor: DeviceDescriptor,
    fan_count: usize,
}

impl Reader {
    pub(super) fn new(descriptor: DeviceDescriptor) -> Self {
        let fan_count = if matches!(descriptor.chip, Chip::F71882 | Chip::F71858) {
            4
        } else {
            3
        };
        Self {
            descriptor,
            fan_count,
        }
    }

    pub(super) fn descriptor(&self) -> DeviceDescriptor {
        self.descriptor
    }

    pub(super) fn read_temperatures(&self, access: &Access<'_>) -> io::Result<Vec<Option<f64>>> {
        const TEMPERATURE_BASE_REG: u8 = 0x70;
        const TEMPERATURE_CONFIG_REG: u8 = 0x69;
        let count = if self.descriptor.chip == Chip::F71808E {
            2
        } else {
            3
        };
        let table_mode = if self.descriptor.chip == Chip::F71858 {
            self.read_byte(access, TEMPERATURE_CONFIG_REG)? & 0x03
        } else {
            0
        };
        let mut values = Vec::with_capacity(count);
        for index in 0..count {
            if self.descriptor.chip == Chip::F71858 {
                let high = self.read_byte(access, TEMPERATURE_BASE_REG + (2 * index) as u8)?;
                let low = self.read_byte(access, TEMPERATURE_BASE_REG + (2 * index) as u8 + 1)?;
                values.push(decode_f71858_temperature(table_mode, high, low));
            } else {
                let raw = self.read_byte(access, TEMPERATURE_BASE_REG + (2 * (index + 1)) as u8)?;
                let temperature = raw as i8;
                values.push(
                    (temperature > 0 && temperature < i8::MAX).then_some(f64::from(temperature)),
                );
            }
        }
        Ok(values)
    }

    pub(super) fn read_fans(&self, access: &Access<'_>) -> io::Result<Vec<Option<u64>>> {
        const FAN_TACHOMETER_REG: [u8; 4] = [0xA0, 0xB0, 0xC0, 0xD0];
        let mut values = Vec::with_capacity(self.fan_count);
        for register in FAN_TACHOMETER_REG.iter().take(self.fan_count) {
            let high = self.read_byte(access, *register)?;
            let low = self.read_byte(access, register.wrapping_add(1))?;
            values.push(decode_fan((u16::from(high) << 8) | u16::from(low)));
        }
        Ok(values)
    }

    fn read_byte(&self, access: &Access<'_>, register: u8) -> io::Result<u8> {
        access.write_port(self.descriptor.base + 0x05, register)?;
        access.read_port(self.descriptor.base + 0x06)
    }
}

fn decode_f71858_temperature(table_mode: u8, high: u8, low: u8) -> Option<f64> {
    if matches!(high, 0xBB | 0xCC) {
        return None;
    }
    let mut bits = match table_mode & 0x03 {
        2 => u16::from(high & 0x80) << 8,
        3 => u16::from(low & 0x01) << 15,
        _ => 0,
    };
    bits |= u16::from(high) << 7;
    bits |= u16::from(low & 0xE0) >> 1;
    Some(f64::from((bits & 0xFFF0) as i16) / 128.0)
}

fn decode_fan(count: u16) -> Option<u64> {
    match count {
        0 => None,
        0x0fff..=u16::MAX => Some(0),
        value => Some((1_500_000.0 / f64::from(value)).round() as u64),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fintek_fan_decode_preserves_zero_rpm_semantics() {
        assert_eq!(decode_fan(0), None);
        assert_eq!(decode_fan(0x0fff), Some(0));
        assert_eq!(decode_fan(1000), Some(1500));
    }

    #[test]
    fn f71858_temperature_decode_matches_lhm_fractional_format() {
        assert_eq!(decode_f71858_temperature(0, 0x32, 0), Some(50.0));
        assert_eq!(decode_f71858_temperature(0, 0xBB, 0), None);
    }
}
