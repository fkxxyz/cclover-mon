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

impl Reader {
    pub(super) fn new(
        descriptor: DeviceDescriptor,
        access: &Access<'_>,
    ) -> io::Result<Option<Self>> {
        let fan_kind = match descriptor.chip {
            Chip::Nct610Xd => FanKind::DirectRpm {
                registers: (0..3).map(|i| 0x030 + (i << 1)).collect(),
                minimum: (1_350_000 / 0x1FFF) as u16,
            },
            Chip::Nct6771F => FanKind::DirectRpm {
                registers: (0..4).map(|i| 0x656 + (i << 1)).collect(),
                minimum: (1_350_000 / 0xFFFF) as u16,
            },
            Chip::Nct6776F => FanKind::DirectRpm {
                registers: (0..5).map(|i| 0x656 + (i << 1)).collect(),
                minimum: (1_350_000 / 0x1FFF) as u16,
            },
            Chip::Nct6779D => FanKind::Count13 {
                registers: FAN_COUNT_REGISTERS[..5].to_vec(),
            },
            Chip::Nct6796Dr
            | Chip::Nct6796Ds
            | Chip::Nct6797D
            | Chip::Nct6798D
            | Chip::Nct6799D
            | Chip::Nct5585D
            | Chip::Nct6701D => FanKind::Count13 {
                registers: FAN_COUNT_REGISTERS.to_vec(),
            },
            Chip::Nct6791D
            | Chip::Nct6792D
            | Chip::Nct6792Da
            | Chip::Nct6793D
            | Chip::Nct6795D
            | Chip::Nct6796D => FanKind::Count13 {
                registers: FAN_COUNT_REGISTERS[..6].to_vec(),
            },
            Chip::Nct6683D | Chip::Nct6686D | Chip::Nct6687D => FanKind::Nct668x,
            Chip::Nct6687Dr => FanKind::Nct6687Dr,
            _ => return Ok(None),
        };

        let reader = Self {
            descriptor,
            fan_kind,
            temperature_channels: temperature_channels(descriptor.chip),
        };
        if !reader.is_nuvoton_vendor(access)? {
            return Ok(None);
        }
        if matches!(reader.fan_kind, FanKind::Nct668x | FanKind::Nct6687Dr) {
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

    pub(super) fn temperature_metadata(
        &self,
    ) -> impl Iterator<Item = (&'static str, &'static str)> + '_ {
        self.temperature_channels
            .iter()
            .filter(|channel| channel.output)
            .map(|channel| (channel.key, channel.name))
    }

    pub(super) fn read_temperatures(&self, access: &Access<'_>) -> io::Result<Vec<Option<f64>>> {
        match self.descriptor.chip {
            Chip::Nct610Xd => self.read_nct610_temperatures(access),
            Chip::Nct6683D | Chip::Nct6686D | Chip::Nct6687D | Chip::Nct6687Dr => {
                self.read_nct668x_temperatures(access)
            }
            Chip::Nct6701D => self.read_nct6701_temperatures(access),
            Chip::Nct6796D
            | Chip::Nct6796Dr
            | Chip::Nct6796Ds
            | Chip::Nct6797D
            | Chip::Nct6798D
            | Chip::Nct6799D
            | Chip::Nct5585D => self.read_modern_routed_temperatures(access),
            _ => self.read_legacy_routed_temperatures(access),
        }
    }

    pub(super) fn read_fans(&self, access: &Access<'_>) -> io::Result<Vec<Option<u64>>> {
        match &self.fan_kind {
            FanKind::DirectRpm { registers, minimum } => registers
                .iter()
                .map(|&register| {
                    let high = self.read_byte(access, register)?;
                    let low = self.read_byte(access, register + 1)?;
                    let rpm = (u16::from(high) << 8) | u16::from(low);
                    Ok(Some(if rpm > *minimum { u64::from(rpm) } else { 0 }))
                })
                .collect(),
            FanKind::Count13 { registers } => registers
                .iter()
                .map(|&register| self.read_13_bit_fan(access, register))
                .collect(),
            FanKind::Nct668x => self.read_direct_rpm_fans(access, &NCT668X_FAN_RPM_REGISTERS),
            FanKind::Nct6687Dr => self.read_direct_rpm_fans(access, &NCT6687DR_FAN_RPM_REGISTERS),
        }
    }

    fn read_direct_rpm_fans(
        &self,
        access: &Access<'_>,
        registers: &[u16],
    ) -> io::Result<Vec<Option<u64>>> {
        registers
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
            .collect()
    }

    fn read_nct610_temperatures(&self, access: &Access<'_>) -> io::Result<Vec<Option<f64>>> {
        self.temperature_channels
            .iter()
            .filter(|channel| channel.output)
            .map(|channel| {
                let value = f64::from(self.read_byte(access, channel.register)? as i8);
                let half = if channel.half_bit >= 0 {
                    f64::from(
                        (self.read_byte(access, channel.half_register)? >> channel.half_bit) & 1,
                    ) * 0.5
                } else {
                    0.0
                };
                Ok(Some(value + half))
            })
            .collect()
    }

    fn read_nct668x_temperatures(&self, access: &Access<'_>) -> io::Result<Vec<Option<f64>>> {
        self.temperature_channels
            .iter()
            .filter(|channel| channel.output)
            .map(|channel| {
                let value = f64::from(self.read_byte(access, channel.register)? as i8);
                let half = f64::from(self.read_byte(access, channel.register + 1)? >> 7) * 0.5;
                Ok(Some(value + half))
            })
            .collect()
    }

    fn read_nct6701_temperatures(&self, access: &Access<'_>) -> io::Result<Vec<Option<f64>>> {
        let mut by_source = BTreeMap::<u8, Option<f64>>::new();
        let mut direct = BTreeMap::<&'static str, Option<f64>>::new();
        for channel in &self.temperature_channels {
            let Some(configured_source) = channel.source else {
                let value = if channel.register == 0 {
                    None
                } else {
                    decode_nct6701_temperature(self.read_byte(access, channel.register)?)
                };
                direct.insert(channel.key, value);
                continue;
            };

            let source = if channel.source_register > 0 {
                self.read_byte(access, channel.source_register)?
            } else {
                configured_source
            };
            if !self
                .temperature_channels
                .iter()
                .any(|candidate| candidate.source == Some(source))
                || by_source.contains_key(&source)
                || channel.register == 0
            {
                continue;
            }
            let value = decode_nct6701_temperature(self.read_byte(access, channel.register)?);
            if value.is_some() {
                by_source.insert(source, value);
            }
        }

        self.apply_alternate_temperatures(access, &mut by_source)?;
        Ok(self
            .temperature_channels
            .iter()
            .filter(|channel| channel.output)
            .map(|channel| match channel.source {
                Some(source) => by_source.get(&source).copied().flatten(),
                None => direct.get(channel.key).copied().flatten(),
            })
            .collect())
    }

    fn read_modern_routed_temperatures(&self, access: &Access<'_>) -> io::Result<Vec<Option<f64>>> {
        let mut by_source = BTreeMap::<u8, Option<f64>>::new();
        for channel in &self.temperature_channels {
            if channel.register == 0 {
                continue;
            }
            let Some(configured_source) = channel.source else {
                continue;
            };
            let mut value = i16::from(self.read_byte(access, channel.register)? as i8) << 1;
            if channel.half_bit > 0 {
                value |= i16::from(
                    (self.read_byte(access, channel.half_register)? >> channel.half_bit) & 1,
                );
            }
            let source = if channel.source_register > 0 {
                self.read_byte(access, channel.source_register)? & 0x1F
            } else {
                configured_source
            };
            if by_source.contains_key(&source) {
                continue;
            }
            let temperature = f64::from(value) * 0.5;
            if (-55.0..=125.0).contains(&temperature) {
                by_source.insert(source, Some(temperature));
            }
        }
        self.apply_alternate_temperatures(access, &mut by_source)?;
        Ok(self
            .temperature_channels
            .iter()
            .filter(|channel| channel.output)
            .map(|channel| {
                channel
                    .source
                    .and_then(|source| by_source.get(&source).copied().flatten())
            })
            .collect())
    }

    fn read_legacy_routed_temperatures(&self, access: &Access<'_>) -> io::Result<Vec<Option<f64>>> {
        let mut by_source = BTreeMap::<u8, Option<f64>>::new();
        for channel in &self.temperature_channels {
            if channel.register == 0 || channel.source_register == 0 {
                continue;
            }
            let mut value = i16::from(self.read_byte(access, channel.register)? as i8) << 1;
            if channel.half_bit > 0 {
                value |= i16::from(
                    (self.read_byte(access, channel.half_register)? >> channel.half_bit) & 1,
                );
            }
            let source = self.read_byte(access, channel.source_register)?;
            let temperature = f64::from(value) * 0.5;
            by_source.insert(
                source,
                (-55.0..=125.0)
                    .contains(&temperature)
                    .then_some(temperature),
            );
        }
        self.apply_alternate_temperatures(access, &mut by_source)?;
        Ok(self
            .temperature_channels
            .iter()
            .filter(|channel| channel.output)
            .map(|channel| {
                channel
                    .source
                    .and_then(|source| by_source.get(&source).copied().flatten())
            })
            .collect())
    }

    fn apply_alternate_temperatures(
        &self,
        access: &Access<'_>,
        by_source: &mut BTreeMap<u8, Option<f64>>,
    ) -> io::Result<()> {
        for channel in &self.temperature_channels {
            let (Some(source), Some(register)) = (channel.source, channel.alternate_register)
            else {
                continue;
            };
            if by_source.get(&source).is_some_and(Option::is_some) {
                continue;
            }
            let temperature = f64::from(self.read_byte(access, register)? as i8);
            by_source.insert(
                source,
                (temperature > 0.0 && temperature <= 125.0).then_some(temperature),
            );
        }
        Ok(())
    }

    fn read_13_bit_fan(&self, access: &Access<'_>, register: u16) -> io::Result<Option<u64>> {
        let high = self.read_byte(access, register)?;
        let low = self.read_byte(access, register + 1)?;
        Ok(decode_13_bit(low, high, false))
    }

    fn is_nuvoton_vendor(&self, access: &Access<'_>) -> io::Result<bool> {
        if matches!(
            self.descriptor.chip,
            Chip::Nct6683D | Chip::Nct6686D | Chip::Nct6687D | Chip::Nct6687Dr | Chip::Nct6701D
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
            Chip::Nct6683D | Chip::Nct6686D | Chip::Nct6687D | Chip::Nct6687Dr
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
            Chip::Nct6683D | Chip::Nct6686D | Chip::Nct6687D | Chip::Nct6687Dr
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

fn temperature_channels(chip: Chip) -> Vec<TemperatureChannel> {
    match chip {
        Chip::Nct610Xd => vec![
            tc("peci0", "PECI #0", Some(12), 0x06B, 0, -1, 0x621, None),
            tc("aux", "Auxiliary", Some(2), 0x010, 0x016, 0, 0, None),
            tc("cpu", "CPU", Some(1), 0x011, 0x01B, 1, 0, None),
            tc("system0", "System #1", Some(3), 0x012, 0x01B, 2, 0, None),
            tc("system1", "System #2", Some(4), 0x013, 0x016, 3, 0, None),
            tc("system2", "System #3", Some(5), 0x014, 0x01B, 4, 0, None),
            tc("system3", "System #4", Some(6), 0x015, 0x01B, 5, 0, None),
        ],
        Chip::Nct6771F => legacy_677x_channels(5),
        Chip::Nct6776F => legacy_677x_channels(12),
        Chip::Nct6683D | Chip::Nct6686D | Chip::Nct6687D => nct668x_channels(),
        Chip::Nct6687Dr => nct6687dr_channels(),
        Chip::Nct6701D => nct6701_channels(),
        Chip::Nct6793D | Chip::Nct6795D | Chip::Nct6791D | Chip::Nct6792D | Chip::Nct6792Da => {
            modern_channels(false)
        }
        Chip::Nct6796D | Chip::Nct6796Dr | Chip::Nct6797D => modern_channels(false),
        Chip::Nct6796Ds => nct6796ds_channels(),
        Chip::Nct6798D | Chip::Nct6799D => modern_channels(true),
        Chip::Nct5585D => nct5585_channels(),
        _ => default_67xx_channels(),
    }
}

const fn tc(
    key: &'static str,
    name: &'static str,
    source: Option<u8>,
    register: u16,
    half_register: u16,
    half_bit: i8,
    source_register: u16,
    alternate_register: Option<u16>,
) -> TemperatureChannel {
    TemperatureChannel::new(
        key,
        name,
        source,
        register,
        half_register,
        half_bit,
        source_register,
        alternate_register,
    )
}

fn legacy_677x_channels(peci_source: u8) -> Vec<TemperatureChannel> {
    vec![
        tc(
            "peci0",
            "PECI #0",
            Some(peci_source),
            0x027,
            0,
            -1,
            0x621,
            None,
        ),
        tc("cpu", "CPU", Some(2), 0x073, 0x074, 7, 0x100, None),
        tc("aux", "Auxiliary", Some(3), 0x075, 0x076, 7, 0x200, None),
        tc("system", "System", Some(1), 0x077, 0x078, 7, 0x300, None),
        TemperatureChannel::helper("route4", 0x150, 0x151, 7, 0x622),
        TemperatureChannel::helper("route5", 0x250, 0x251, 7, 0x623),
        TemperatureChannel::helper("route6", 0x62B, 0x62E, 0, 0x624),
        TemperatureChannel::helper("route7", 0x62C, 0x62E, 1, 0x625),
        TemperatureChannel::helper("route8", 0x62D, 0x62E, 2, 0x626),
    ]
}

fn default_67xx_channels() -> Vec<TemperatureChannel> {
    vec![
        tc("peci0", "PECI #0", Some(16), 0x027, 0, -1, 0x621, None),
        tc("cpu", "CPU", Some(2), 0x073, 0x074, 7, 0x100, Some(0x491)),
        tc(
            "system",
            "System",
            Some(1),
            0x075,
            0x076,
            7,
            0x200,
            Some(0x490),
        ),
        tc(
            "aux0",
            "Auxiliary #1",
            Some(3),
            0x077,
            0x078,
            7,
            0x300,
            Some(0x492),
        ),
        tc(
            "aux1",
            "Auxiliary #2",
            Some(4),
            0x079,
            0x07A,
            7,
            0x800,
            Some(0x493),
        ),
        tc(
            "aux2",
            "Auxiliary #3",
            Some(5),
            0x07B,
            0x07C,
            7,
            0x900,
            Some(0x494),
        ),
        tc(
            "aux3",
            "Auxiliary #4",
            Some(6),
            0x150,
            0x151,
            7,
            0x622,
            Some(0x495),
        ),
    ]
}

fn modern_channels(include_tsensor: bool) -> Vec<TemperatureChannel> {
    let mut values = vec![
        tc("peci0", "PECI #0", Some(16), 0x073, 0x074, 7, 0x100, None),
        tc("cpu", "CPU", Some(2), 0x075, 0x076, 7, 0x200, Some(0x491)),
        tc(
            "system",
            "System",
            Some(1),
            0x077,
            0x078,
            7,
            0x300,
            Some(0x490),
        ),
        tc(
            "aux0",
            "Auxiliary #1",
            Some(3),
            0x079,
            0x07A,
            7,
            0x800,
            Some(0x492),
        ),
        tc(
            "aux1",
            "Auxiliary #2",
            Some(4),
            0x07B,
            0x07C,
            7,
            0x900,
            Some(0x493),
        ),
        tc(
            "aux2",
            "Auxiliary #3",
            Some(5),
            0x07D,
            0x07E,
            7,
            0xA00,
            Some(0x494),
        ),
        tc(
            "aux3",
            "Auxiliary #4",
            Some(6),
            0x4A0,
            0x49E,
            6,
            0xB00,
            Some(0x495),
        ),
        tc("aux4", "Auxiliary #5", Some(7), 0x027, 0, -1, 0x621, None),
    ];
    if include_tsensor {
        values.push(tc(
            "tsensor",
            "T Sensor",
            Some(10),
            0x4A2,
            0x4A1,
            7,
            0xC00,
            Some(0x496),
        ));
    }
    values.extend([
        tc("smbus0", "SMBus #0", Some(8), 0x150, 0x151, 7, 0x622, None),
        tc("smbus1", "SMBus #1", Some(9), 0x670, 0, -1, 0xC26, None),
        tc("peci1", "PECI #1", Some(17), 0x672, 0, -1, 0xC27, None),
        tc(
            "pch-cpu-max",
            "PCH CPU Max",
            Some(18),
            0x674,
            0,
            -1,
            0xC28,
            Some(0x400),
        ),
        tc(
            "pch-chip",
            "PCH Chip",
            Some(19),
            0x676,
            0,
            -1,
            0xC29,
            Some(0x401),
        ),
        tc(
            "pch-cpu",
            "PCH CPU",
            Some(20),
            0x678,
            0,
            -1,
            0xC2A,
            Some(0x402),
        ),
        tc(
            "pch-mch",
            "PCH MCH",
            Some(21),
            0x67A,
            0,
            -1,
            0xC2B,
            Some(0x404),
        ),
        tc("dimm-a0", "DIMM A0", Some(22), 0x405, 0, -1, 0, None),
        tc("dimm-a1", "DIMM A1", Some(23), 0x406, 0, -1, 0, None),
        tc("dimm-b0", "DIMM B0", Some(24), 0x407, 0, -1, 0, None),
        tc("dimm-b1", "DIMM B1", Some(25), 0x408, 0, -1, 0, None),
        tc(
            "byte0",
            "Byte Temperature #1",
            Some(26),
            0x419,
            0,
            -1,
            0,
            None,
        ),
        tc(
            "byte1",
            "Byte Temperature #2",
            Some(27),
            0x41A,
            0,
            -1,
            0,
            None,
        ),
        tc("peci0-cal", "PECI #0 Cal", Some(28), 0x4F4, 0, -1, 0, None),
        tc("peci1-cal", "PECI #1 Cal", Some(29), 0x4F5, 0, -1, 0, None),
        tc("virtual", "Virtual", Some(31), 0, 0, -1, 0, None),
        tc("spare0", "Spare #1", Some(32), 0, 0, -1, 0, None),
        tc("spare1", "Spare #2", Some(33), 0, 0, -1, 0, None),
    ]);
    values
}

fn nct6701_channels() -> Vec<TemperatureChannel> {
    vec![
        tc("peci0", "PECI #0", Some(16), 0x073, 0, -1, 0x100, None),
        tc("cpu", "CPU", Some(2), 0x491, 0, -1, 0, None),
        tc("system", "System", Some(1), 0x490, 0, -1, 0, None),
        tc("aux0", "Auxiliary #1", Some(3), 0x492, 0, -1, 0, None),
        tc("aux1", "Auxiliary #2", Some(4), 0x493, 0, -1, 0, None),
        tc("aux2", "Auxiliary #3", Some(5), 0x494, 0, -1, 0, None),
        tc("aux3", "Auxiliary #4", Some(6), 0x495, 0, -1, 0, None),
        tc("aux4", "Auxiliary #5", Some(7), 0x027, 0, -1, 0x621, None),
        tc("peci1", "PECI #1", Some(17), 0x672, 0, -1, 0xC27, None),
        tc(
            "pch-cpu-max",
            "PCH CPU Max",
            Some(18),
            0x674,
            0,
            -1,
            0xC28,
            Some(0x400),
        ),
        tc(
            "pch-chip",
            "PCH Chip",
            Some(19),
            0x676,
            0,
            -1,
            0xC29,
            Some(0x401),
        ),
        tc(
            "pch-cpu",
            "PCH CPU",
            Some(20),
            0x678,
            0,
            -1,
            0xC2A,
            Some(0x402),
        ),
        tc(
            "pch-mch",
            "PCH MCH",
            Some(21),
            0x67A,
            0,
            -1,
            0xC2B,
            Some(0x404),
        ),
        tc("dimm-a0", "DIMM A0", Some(22), 0x405, 0, -1, 0, None),
        tc("dimm-a1", "DIMM A1", Some(23), 0x406, 0, -1, 0, None),
        tc("dimm-b0", "DIMM B0", Some(24), 0x407, 0, -1, 0, None),
        tc("dimm-b1", "DIMM B1", Some(25), 0x408, 0, -1, 0, None),
        tc("smbus0", "SMBus #0", Some(8), 0x150, 0, -1, 0x622, None),
        tc("smbus1", "SMBus #1", Some(9), 0x670, 0, -1, 0xC26, None),
        tc(
            "byte0",
            "Byte Temperature #1",
            Some(26),
            0x419,
            0,
            -1,
            0,
            None,
        ),
        tc(
            "byte1",
            "Byte Temperature #2",
            Some(27),
            0x41A,
            0,
            -1,
            0,
            None,
        ),
        tc("peci0-cal", "PECI #0 Cal", Some(28), 0x4F4, 0, -1, 0, None),
        tc("peci1-cal", "PECI #1 Cal", Some(29), 0x4F5, 0, -1, 0, None),
        tc("virtual", "Virtual", Some(31), 0, 0, -1, 0, None),
        tc("spare0", "Spare #1", Some(32), 0x07B, 0, -1, 0x900, None),
        tc("spare1", "Spare #2", Some(33), 0, 0, -1, 0, None),
        tc("cpu-package", "CPU Package", None, 0x409, 0, -1, 0, None),
        tc("temp14", "Temperature #14", None, 0x4A2, 0, -1, 0, None),
    ]
}

fn nct6796ds_channels() -> Vec<TemperatureChannel> {
    vec![
        tc("cpu", "CPU", Some(2), 0x073, 0x074, 7, 0x100, Some(0x491)),
        tc(
            "system",
            "System",
            Some(1),
            0x075,
            0x076,
            7,
            0x200,
            Some(0x490),
        ),
        tc(
            "aux0",
            "Auxiliary #1",
            Some(3),
            0x077,
            0x078,
            7,
            0x300,
            Some(0x492),
        ),
        tc(
            "aux1",
            "Auxiliary #2",
            Some(4),
            0x079,
            0x07A,
            7,
            0x800,
            Some(0x493),
        ),
        tc(
            "aux2",
            "Auxiliary #3",
            Some(5),
            0x07B,
            0x07C,
            7,
            0x900,
            Some(0x494),
        ),
        tc(
            "aux3",
            "Auxiliary #4",
            Some(6),
            0x07D,
            0x07E,
            7,
            0xA00,
            Some(0x495),
        ),
        tc(
            "aux4",
            "Auxiliary #5",
            Some(7),
            0x027,
            0,
            4,
            0xC16,
            Some(0x496),
        ),
        tc(
            "aux5",
            "Auxiliary #6",
            Some(34),
            0x449,
            0,
            4,
            0x100,
            Some(0x4A2),
        ),
        tc("smbus0", "SMBus #0", Some(8), 0x150, 0x151, 7, 0x622, None),
        tc("peci0", "PECI #0", Some(16), 0x720, 0, -1, 0, None),
        tc("virtual", "Virtual", Some(31), 0, 0, -1, 0, None),
    ]
}

fn nct5585_channels() -> Vec<TemperatureChannel> {
    vec![
        tc("peci0", "PECI #0", Some(16), 0x720, 0, -1, 0x100, None),
        tc("cpu", "CPU", Some(2), 0x075, 0x076, 7, 0, Some(0x073)),
        tc(
            "aux1",
            "Auxiliary #2",
            Some(4),
            0x07B,
            0x07C,
            7,
            0x900,
            Some(0x493),
        ),
        tc(
            "aux3",
            "Auxiliary #4",
            Some(6),
            0x4A0,
            0x49E,
            6,
            0xB00,
            Some(0x495),
        ),
    ]
}

fn nct6687dr_channels() -> Vec<TemperatureChannel> {
    [
        ("cpu", "CPU"),
        ("system", "System"),
        ("mos", "MOS"),
        ("pch", "PCH"),
        ("cpu-socket", "CPU Socket"),
        ("pcie1", "PCIe #1"),
        ("m2-1", "M2 #1"),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (key, name))| tc(key, name, None, 0x100 + index as u16 * 2, 0, -1, 0, None))
    .collect()
}

fn nct668x_channels() -> Vec<TemperatureChannel> {
    let names = [
        ("cpu", "CPU"),
        ("system", "System"),
        ("mos", "MOS"),
        ("pch", "PCH"),
        ("cpu-socket", "CPU Socket"),
        ("pcie1", "PCIe #1"),
        ("m2-1", "M2 #1"),
        ("pcie2", "PCIe #2"),
        ("pcie3", "PCIe #3"),
        ("m2-2", "M2 #2"),
        ("m2-4", "M2 #4"),
    ];
    names
        .into_iter()
        .enumerate()
        .map(|(index, (key, name))| tc(key, name, None, 0x100 + (index as u16 * 2), 0, -1, 0, None))
        .collect()
}

fn decode_nct6701_temperature(raw: u8) -> Option<f64> {
    if raw == 0 || raw == 0xA0 || (0x7E..=0x80).contains(&raw) {
        None
    } else {
        Some(f64::from(raw as i8))
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
