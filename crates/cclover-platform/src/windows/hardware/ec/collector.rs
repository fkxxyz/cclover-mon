use super::*;

impl Collector {
    pub(in crate::windows::hardware) fn new(board: &board::Info) -> Option<Self> {
        if !board.is_asus() {
            return None;
        }
        let board_key = board.product_key();
        let config = board_config(&board_key)?;
        Some(Self {
            board_key,
            config,
            wait_read_failures: 0,
        })
    }

    pub(in crate::windows::hardware) fn supports_temperatures(&self) -> bool {
        self.config.sensors.iter().any(|sensor| {
            sensor_spec(self.config.family, *sensor)
                .is_some_and(|spec| matches!(spec.kind, Kind::Temperature))
        })
    }

    pub(in crate::windows::hardware) fn supports_fans(&self) -> bool {
        self.config.sensors.iter().any(|sensor| {
            sensor_spec(self.config.family, *sensor)
                .is_some_and(|spec| matches!(spec.kind, Kind::Fan))
        })
    }

    pub(in crate::windows::hardware) fn collect(
        &mut self,
        session: &Session,
        projection: Projection,
    ) -> io::Result<Observation> {
        let specs = self
            .config
            .sensors
            .iter()
            .filter_map(|sensor| sensor_spec(self.config.family, *sensor))
            .filter(|spec| match spec.kind {
                Kind::Temperature => projection.temperatures,
                Kind::Fan => projection.fans,
            })
            .collect::<Vec<_>>();
        if specs.is_empty() {
            return Ok(Observation {
                temperatures: Vec::new(),
                fans: Vec::new(),
                temperature_degraded: false,
                fan_degraded: false,
            });
        }

        let mut indexed = specs
            .iter()
            .enumerate()
            .flat_map(|(sensor_index, spec)| {
                (0..spec.size)
                    .map(move |byte| (spec.register + u16::from(byte), sensor_index, byte))
            })
            .collect::<Vec<_>>();
        indexed.sort_unstable_by_key(|entry| entry.0);
        let registers = indexed.iter().map(|entry| entry.0).collect::<Vec<_>>();
        let bytes = self.read_registers(session, &registers)?;

        let mut sensor_bytes = specs
            .iter()
            .map(|spec| vec![0_u8; usize::from(spec.size)])
            .collect::<Vec<_>>();
        for ((_, sensor_index, byte), value) in indexed.into_iter().zip(bytes) {
            sensor_bytes[sensor_index][usize::from(byte)] = value;
        }

        let mut temperatures = Vec::new();
        let mut fans = Vec::new();
        let mut temperature_degraded = false;
        let fan_degraded = false;
        for (spec, bytes) in specs.into_iter().zip(sensor_bytes) {
            match spec.kind {
                Kind::Temperature => {
                    let raw = i16::from(bytes[0] as i8);
                    if spec.blank == Some(raw) {
                        continue;
                    }
                    if (-55..=125).contains(&raw) {
                        temperatures.push(TemperatureSnapshot {
                            id: TemperatureId::from_opaque_key(format!(
                                "windows:ec:{}:{}",
                                self.board_key, spec.key
                            )),
                            name: spec.name.to_owned(),
                            celsius: f64::from(raw),
                        });
                    } else {
                        temperature_degraded = true;
                    }
                }
                Kind::Fan => {
                    let raw = u16::from_be_bytes([bytes[0], bytes[1]]);
                    fans.push(FanSnapshot {
                        id: FanId::from_opaque_key(format!(
                            "windows:ec:{}:{}",
                            self.board_key, spec.key
                        )),
                        name: spec.name.to_owned(),
                        rpm: u64::from(raw),
                    });
                }
            }
        }
        Ok(Observation {
            temperatures,
            fans,
            temperature_degraded,
            fan_degraded,
        })
    }

    fn read_registers(&mut self, session: &Session, registers: &[u16]) -> io::Result<Vec<u8>> {
        let _guard = lock_named(EC_MUTEX, EC_WAIT_MS, "EC")?;
        let previous_bank = self.read_byte(session, 0xFF)?;
        self.write_byte(session, 0xFF, 0)?;

        let result = (|| {
            let mut bank = 0_u8;
            let mut output = Vec::with_capacity(registers.len());
            for &register in registers {
                let next_bank = (register >> 8) as u8;
                if next_bank != bank {
                    self.write_byte(session, 0xFF, next_bank)?;
                    bank = next_bank;
                }
                output.push(self.read_byte(session, register as u8)?);
            }
            Ok(output)
        })();

        let restore = self.write_byte(session, 0xFF, previous_bank);
        match (result, restore) {
            (Ok(values), Ok(())) => Ok(values),
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
        }
    }

    fn read_byte(&mut self, session: &Session, register: u8) -> io::Result<u8> {
        for _ in 0..MAX_RETRIES {
            if let Some(value) = self.read_byte_once(session, register)? {
                return Ok(value);
            }
        }
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            format!("EC register 0x{register:02x} read timed out"),
        ))
    }

    fn write_byte(&mut self, session: &Session, register: u8, value: u8) -> io::Result<()> {
        for _ in 0..MAX_RETRIES {
            if self.write_byte_once(session, register, value)? {
                return Ok(());
            }
        }
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            format!("EC register 0x{register:02x} write timed out"),
        ))
    }

    fn read_byte_once(&mut self, session: &Session, register: u8) -> io::Result<Option<u8>> {
        if !self.wait_write(session)? {
            return Ok(None);
        }
        self.write_port(session, COMMAND_PORT, READ_COMMAND)?;
        if !self.wait_write(session)? {
            return Ok(None);
        }
        self.write_port(session, DATA_PORT, register)?;
        if !self.wait_write(session)? || !self.wait_read(session)? {
            return Ok(None);
        }
        self.read_port(session, DATA_PORT).map(Some)
    }

    fn write_byte_once(&mut self, session: &Session, register: u8, value: u8) -> io::Result<bool> {
        if !self.wait_write(session)? {
            return Ok(false);
        }
        self.write_port(session, COMMAND_PORT, WRITE_COMMAND)?;
        if !self.wait_write(session)? {
            return Ok(false);
        }
        self.write_port(session, DATA_PORT, register)?;
        if !self.wait_write(session)? {
            return Ok(false);
        }
        self.write_port(session, DATA_PORT, value)?;
        Ok(true)
    }

    fn wait_write(&self, session: &Session) -> io::Result<bool> {
        for _ in 0..WAIT_SPINS {
            if self.read_port(session, COMMAND_PORT)? & INPUT_BUFFER_FULL == 0 {
                return Ok(true);
            }
            thread::sleep(Duration::from_millis(1));
        }
        Ok(false)
    }

    fn wait_read(&mut self, session: &Session) -> io::Result<bool> {
        if self.wait_read_failures > FAILURES_BEFORE_SKIP {
            return Ok(true);
        }
        for _ in 0..MAX_RETRIES {
            if self.read_port(session, COMMAND_PORT)? & OUTPUT_BUFFER_FULL != 0 {
                self.wait_read_failures = 0;
                return Ok(true);
            }
            thread::sleep(Duration::from_millis(1));
        }
        // LHM ASUS workaround: some ECs expose readiness by clearing IBF rather than setting OBF.
        for _ in 0..WAIT_SPINS {
            if self.read_port(session, COMMAND_PORT)? & INPUT_BUFFER_FULL == 0 {
                self.wait_read_failures = 0;
                return Ok(true);
            }
            thread::sleep(Duration::from_millis(1));
        }
        self.wait_read_failures += 1;
        Ok(false)
    }

    fn read_port(&self, session: &Session, port: u8) -> io::Result<u8> {
        session
            .execute("ioctl_pio_read", &[u64::from(port)], 1)?
            .first()
            .copied()
            .map(|value| value as u8)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "PawnIO EC read returned no value",
                )
            })
    }

    fn write_port(&self, session: &Session, port: u8, value: u8) -> io::Result<()> {
        session
            .execute("ioctl_pio_write", &[u64::from(port), u64::from(value)], 0)
            .map(|_| ())
    }
}
