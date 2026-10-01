// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// https://mozilla.org/MPL/2.0/.
// Copyright (C) LibreHardwareMonitor and Contributors.
// Partial Copyright (C) Michael Möller <mmoeller@openhardwaremonitor.org> and Contributors.
// Temperature/fan decode ported for cclover-mon from LibreHardwareMonitor Nct677X.cs at reviewed commit
// 677a3a56abde9adff5abdb42db3a7638c9137572.

use std::collections::BTreeMap;
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
const NCT6687DR_FAN_RPM_REGISTERS: [u16; 16] = [
    0x140, 0x142, 0x144, 0x146, 0x148, 0x14A, 0x14C, 0x14E, 0x150, 0x152, 0x15E, 0x15C, 0x15A,
    0x158, 0x156, 0x154,
];

pub(super) struct Reader {
    descriptor: DeviceDescriptor,
    fan_kind: FanKind,
    temperature_channels: Vec<TemperatureChannel>,
}

enum FanKind {
    DirectRpm { registers: Vec<u16>, minimum: u16 },
    Count13 { registers: Vec<u16> },
    Nct668x,
    Nct6687Dr,
}

#[derive(Clone, Copy)]
struct TemperatureChannel {
    key: &'static str,
    name: &'static str,
    source: Option<u8>,
    register: u16,
    half_register: u16,
    half_bit: i8,
    source_register: u16,
    alternate_register: Option<u16>,
    output: bool,
}

impl TemperatureChannel {
    const fn new(
        key: &'static str,
        name: &'static str,
        source: Option<u8>,
        register: u16,
        half_register: u16,
        half_bit: i8,
        source_register: u16,
        alternate_register: Option<u16>,
    ) -> Self {
        Self {
            key,
            name,
            source,
            register,
            half_register,
            half_bit,
            source_register,
            alternate_register,
            output: true,
        }
    }

    const fn helper(
        key: &'static str,
        register: u16,
        half_register: u16,
        half_bit: i8,
        source_register: u16,
    ) -> Self {
        Self {
            key,
            name: "",
            source: None,
            register,
            half_register,
            half_bit,
            source_register,
            alternate_register: None,
            output: false,
        }
    }
}

mod channels;
mod decode;
mod reader;

use channels::temperature_channels;
use decode::{decode_13_bit, decode_nct6701_temperature};

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

    #[test]
    fn nct6701_temperature_sentinels_match_lhm() {
        assert_eq!(decode_nct6701_temperature(0), None);
        assert_eq!(decode_nct6701_temperature(0x7F), None);
        assert_eq!(decode_nct6701_temperature(0xA0), None);
        assert_eq!(decode_nct6701_temperature(42), Some(42.0));
    }

    #[test]
    fn topology_is_fixed_by_chip_not_runtime_values() {
        assert_eq!(nct668x_channels().len(), 11);
        assert_eq!(temperature_channels(Chip::Nct610Xd).len(), 7);
        assert!(
            temperature_channels(Chip::Nct6798D)
                .iter()
                .any(|channel| channel.key == "tsensor")
        );
    }
}
