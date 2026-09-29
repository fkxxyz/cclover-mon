// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// https://mozilla.org/MPL/2.0/.
// Copyright (C) LibreHardwareMonitor and Contributors.
// Partial Copyright (C) Michael Möller <mmoeller@openhardwaremonitor.org> and Contributors.
// Fan decode ported for cclover-mon from LibreHardwareMonitor F718XX.cs at reviewed commit
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
}
