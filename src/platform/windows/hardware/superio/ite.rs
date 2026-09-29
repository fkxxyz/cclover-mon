// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// https://mozilla.org/MPL/2.0/.
// Copyright (C) LibreHardwareMonitor and Contributors.
// Partial Copyright (C) Michael Möller <mmoeller@openhardwaremonitor.org> and Contributors.
// Fan decode ported for cclover-mon from LibreHardwareMonitor IT87XX.cs at reviewed commit
// 677a3a56abde9adff5abdb42db3a7638c9137572.

use std::io;

use super::access::Access;
use super::chip::Chip;
use super::detect::DeviceDescriptor;

const FAN_TACHOMETER_REG: [u8; 6] = [0x0D, 0x0E, 0x0F, 0x80, 0x82, 0x4C];
const FAN_TACHOMETER_EXT_REG: [u8; 6] = [0x18, 0x19, 0x1A, 0x81, 0x83, 0x4D];
const FAN_TACHOMETER_REG_ALT: [u8; 6] = [0x0D, 0x0E, 0x0F, 0x80, 0x82, 0x93];
const FAN_TACHOMETER_EXT_REG_ALT: [u8; 6] = [0x18, 0x19, 0x1A, 0x81, 0x83, 0x94];

pub(super) struct Reader {
    descriptor: DeviceDescriptor,
    fan_count: usize,
    has_16_bit_counter: bool,
    alt_sixth_fan_register: bool,
    disabled: [bool; 6],
    requires_bank_zero: bool,
}

impl Reader {
    pub(super) fn new(
        descriptor: DeviceDescriptor,
        access: &Access<'_>,
    ) -> io::Result<Option<Self>> {
        let fan_count = fan_count(descriptor.chip);
        let has_16_bit_counter = (descriptor.chip != Chip::It8705F || descriptor.ite_version >= 3)
            && (descriptor.chip != Chip::It8712F || descriptor.ite_version >= 8);
        let alt_sixth_fan_register = matches!(descriptor.chip, Chip::It8665E | Chip::It8625E);
        let requires_bank_zero = matches!(descriptor.chip, Chip::It8655E | Chip::It8665E);

        let reader = Self {
            descriptor,
            fan_count,
            has_16_bit_counter,
            alt_sixth_fan_register,
            disabled: [false; 6],
            requires_bank_zero,
        };
        let vendor = reader.read_byte(access, 0x58)?.0;
        if !matches!(vendor, 0x90 | 0x7F) {
            return Ok(None);
        }
        let configuration = reader.read_byte(access, 0x00)?.0;
        if configuration & 0x10 == 0 && !matches!(descriptor.chip, Chip::It8655E | Chip::It8665E) {
            return Ok(None);
        }

        let mut reader = reader;
        if has_16_bit_counter {
            let modes = reader.read_byte(access, 0x0C)?.0;
            if fan_count >= 5 {
                reader.disabled[3] = modes & (1 << 4) == 0;
                reader.disabled[4] = modes & (1 << 5) == 0;
            }
            if fan_count >= 6 {
                if descriptor.chip == Chip::It8665E {
                    let alt_modes = reader.read_byte(access, 0x0B)?.0;
                    reader.disabled[5] = alt_modes & (1 << 3) == 0;
                } else {
                    reader.disabled[5] = modes & (1 << 2) == 0;
                }
            }
        }
        Ok(Some(reader))
    }

    pub(super) fn descriptor(&self) -> DeviceDescriptor {
        self.descriptor
    }

    pub(super) fn read_fans(&self, access: &Access<'_>) -> io::Result<Vec<Option<u64>>> {
        if self.requires_bank_zero {
            self.select_bank_zero(access)?;
        }
        let mut values = Vec::with_capacity(self.fan_count);
        if self.has_16_bit_counter {
            let low_regs = if self.alt_sixth_fan_register {
                &FAN_TACHOMETER_REG_ALT
            } else {
                &FAN_TACHOMETER_REG
            };
            let high_regs = if self.alt_sixth_fan_register {
                &FAN_TACHOMETER_EXT_REG_ALT
            } else {
                &FAN_TACHOMETER_EXT_REG
            };
            for index in 0..self.fan_count {
                if self.disabled[index] {
                    values.push(None);
                    continue;
                }
                let low = self.read_byte(access, low_regs[index])?.0;
                let high = self.read_byte(access, high_regs[index])?.0;
                values.push(decode_16_bit((u16::from(high) << 8) | u16::from(low)));
            }
        } else {
            let divisors = self.read_byte(access, 0x0B)?.0;
            for index in 0..self.fan_count {
                let count = self.read_byte(access, FAN_TACHOMETER_REG[index])?.0;
                let divisor = if index < 2 {
                    1_u32 << ((divisors >> (3 * index)) & 0x7)
                } else {
                    2
                };
                values.push(decode_8_bit(count, divisor));
            }
        }
        Ok(values)
    }

    fn select_bank_zero(&self, access: &Access<'_>) -> io::Result<()> {
        let (value, _) = self.read_byte(access, 0x06)?;
        let selected = value & 0x9F;
        self.write_byte(access, 0x06, selected)
    }

    fn read_byte(&self, access: &Access<'_>, register: u8) -> io::Result<(u8, bool)> {
        let address = self.descriptor.base + 0x05;
        let data = self.descriptor.base + 0x06;
        access.write_port(address, register)?;
        let value = access.read_port(data)?;
        let valid = self.descriptor.chip == Chip::It8688E || access.read_port(address)? == register;
        Ok((value, valid))
    }

    fn write_byte(&self, access: &Access<'_>, register: u8, value: u8) -> io::Result<()> {
        let address = self.descriptor.base + 0x05;
        let data = self.descriptor.base + 0x06;
        access.write_port(address, register)?;
        access.write_port(data, value)?;
        let _ = access.read_port(address)?;
        Ok(())
    }
}

fn fan_count(chip: Chip) -> usize {
    match chip {
        Chip::It8613E => 5,
        Chip::It8625E
        | Chip::It8628E
        | Chip::It8665E
        | Chip::It8686E
        | Chip::It8688E
        | Chip::It8689E
        | Chip::It8696E => 6,
        Chip::It8631E | Chip::It8638E => 2,
        Chip::It87952E | Chip::It8655E | Chip::It8792E | Chip::It8705F => 3,
        Chip::It8620E => 5,
        _ => 5,
    }
}

fn decode_16_bit(value: u16) -> Option<u64> {
    if value <= 0x3F {
        None
    } else if value == 0xFFFF {
        Some(0)
    } else {
        Some((1_350_000.0 / (f64::from(value) * 2.0)).round() as u64)
    }
}

fn decode_8_bit(value: u8, divisor: u32) -> Option<u64> {
    if value == 0 {
        None
    } else if value == 0xFF {
        Some(0)
    } else {
        Some((1_350_000.0 / (f64::from(value) * f64::from(divisor))).round() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ite_fan_decoders_match_lhm_semantics() {
        assert_eq!(decode_16_bit(0x3F), None);
        assert_eq!(decode_16_bit(0xFFFF), Some(0));
        assert_eq!(decode_16_bit(675), Some(1000));
        assert_eq!(decode_8_bit(0xFF, 2), Some(0));
        assert_eq!(decode_8_bit(135, 10), Some(1000));
    }
}
