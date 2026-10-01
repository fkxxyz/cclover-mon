use super::*;

impl Reader {
    pub(in crate::windows::hardware::superio) fn new(
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

    pub(in crate::windows::hardware::superio) fn descriptor(&self) -> DeviceDescriptor {
        self.descriptor
    }

    pub(in crate::windows::hardware::superio) fn temperature_metadata(
        &self,
    ) -> impl Iterator<Item = (&'static str, &'static str)> + '_ {
        self.temperature_channels
            .iter()
            .filter(|channel| channel.output)
            .map(|channel| (channel.key, channel.name))
    }

    pub(in crate::windows::hardware::superio) fn read_temperatures(
        &self,
        access: &Access<'_>,
    ) -> io::Result<Vec<Option<f64>>> {
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

    pub(in crate::windows::hardware::superio) fn read_fans(
        &self,
        access: &Access<'_>,
    ) -> io::Result<Vec<Option<u64>>> {
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
