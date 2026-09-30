// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// https://mozilla.org/MPL/2.0/.
// Copyright (C) LibreHardwareMonitor and Contributors.
// ACPI EC board mappings and transaction protocol follow LibreHardwareMonitor
// EmbeddedController.cs / WindowsEmbeddedControllerIO.cs at reviewed commit
// 677a3a56abde9adff5abdb42db3a7638c9137572.

use std::io;
use std::thread;
use std::time::Duration;

use crate::core::model::{FanId, FanSnapshot, TemperatureId, TemperatureSnapshot};

use super::super::pawnio::Session;
use super::board;
use super::sync::lock_named;

const EC_MUTEX: &str = "Global\\Access_EC";
const EC_WAIT_MS: u32 = 10;
const COMMAND_PORT: u8 = 0x66;
const DATA_PORT: u8 = 0x62;
const READ_COMMAND: u8 = 0x80;
const WRITE_COMMAND: u8 = 0x81;
const OUTPUT_BUFFER_FULL: u8 = 0x01;
const INPUT_BUFFER_FULL: u8 = 0x02;
const MAX_RETRIES: usize = 5;
const WAIT_SPINS: usize = 50;
const FAILURES_BEFORE_SKIP: usize = 20;

#[derive(Clone, Copy)]
pub(super) struct Projection {
    pub temperatures: bool,
    pub fans: bool,
}

pub(super) struct Observation {
    pub temperatures: Vec<TemperatureSnapshot>,
    pub fans: Vec<FanSnapshot>,
    pub temperature_degraded: bool,
    pub fan_degraded: bool,
}

pub(super) struct Collector {
    board_key: String,
    config: BoardConfig,
    wait_read_failures: usize,
}

#[derive(Clone, Copy)]
struct BoardConfig {
    family: Family,
    sensors: &'static [Sensor],
}

#[derive(Clone, Copy)]
enum Family {
    Amd400,
    Amd500,
    Amd600,
    Amd800,
    Intel100,
    Intel300,
    Intel370,
    Intel400,
    Intel600,
    Intel700,
    Intel800,
}

#[allow(clippy::enum_variant_names)]
#[derive(Clone, Copy)]
enum Sensor {
    TempChipset,
    TempCPU,
    TempCPUPackage,
    TempMB,
    TempTSensor,
    TempTSensorAlt,
    TempTSensor2,
    TempVrm,
    TempWaterIn,
    TempWaterOut,
    TempWaterBlockIn,
    FanCPUOpt,
    FanVrmHS,
    FanChipset,
    FanWaterPump,
}

#[derive(Clone, Copy)]
enum Kind {
    Temperature,
    Fan,
}

#[derive(Clone, Copy)]
struct SensorSpec {
    key: &'static str,
    name: &'static str,
    kind: Kind,
    register: u16,
    size: u8,
    blank: Option<i16>,
}

impl Collector {
    pub(super) fn new(board: &board::Info) -> Option<Self> {
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

    pub(super) fn supports_temperatures(&self) -> bool {
        self.config.sensors.iter().any(|sensor| {
            sensor_spec(self.config.family, *sensor)
                .is_some_and(|spec| matches!(spec.kind, Kind::Temperature))
        })
    }

    pub(super) fn supports_fans(&self) -> bool {
        self.config.sensors.iter().any(|sensor| {
            sensor_spec(self.config.family, *sensor)
                .is_some_and(|spec| matches!(spec.kind, Kind::Fan))
        })
    }

    pub(super) fn collect(
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

fn board_config(product_key: &str) -> Option<BoardConfig> {
    match product_key {
        "rogstrixb850egamingwifi" => Some(BoardConfig {
            family: Family::Amd800,
            sensors: &[
                Sensor::TempCPUPackage,
                Sensor::TempVrm,
                Sensor::TempTSensorAlt,
            ],
        }),
        "rogcrosshairx870edarkhero" => Some(BoardConfig {
            family: Family::Amd800,
            sensors: &[Sensor::TempTSensor],
        }),
        "rogcrosshairx870eherobtf" => Some(BoardConfig {
            family: Family::Amd800,
            sensors: &[
                Sensor::TempCPU,
                Sensor::TempCPUPackage,
                Sensor::TempMB,
                Sensor::TempVrm,
                Sensor::TempTSensor,
                Sensor::FanCPUOpt,
            ],
        }),
        "rogcrosshairx870ehero" => Some(BoardConfig {
            family: Family::Amd800,
            sensors: &[
                Sensor::TempCPU,
                Sensor::TempCPUPackage,
                Sensor::TempMB,
                Sensor::TempVrm,
                Sensor::TempTSensor,
                Sensor::FanCPUOpt,
            ],
        }),
        "tufgamingx870pluswifi" => Some(BoardConfig {
            family: Family::Amd800,
            sensors: &[Sensor::TempVrm, Sensor::FanCPUOpt],
        }),
        "rogstrixx870eegamingwifi" => Some(BoardConfig {
            family: Family::Amd800,
            sensors: &[
                Sensor::TempCPU,
                Sensor::TempCPUPackage,
                Sensor::TempMB,
                Sensor::TempVrm,
                Sensor::FanCPUOpt,
            ],
        }),
        "primex470pro" => Some(BoardConfig {
            family: Family::Amd400,
            sensors: &[
                Sensor::TempChipset,
                Sensor::TempCPU,
                Sensor::TempMB,
                Sensor::TempVrm,
                Sensor::FanCPUOpt,
            ],
        }),
        "primex570pro" => Some(BoardConfig {
            family: Family::Amd500,
            sensors: &[
                Sensor::TempChipset,
                Sensor::TempCPU,
                Sensor::TempMB,
                Sensor::TempVrm,
                Sensor::TempTSensor,
                Sensor::FanChipset,
            ],
        }),
        "proartx570creatorwifi" => Some(BoardConfig {
            family: Family::Amd500,
            sensors: &[
                Sensor::TempChipset,
                Sensor::TempCPU,
                Sensor::TempMB,
                Sensor::TempVrm,
                Sensor::TempTSensor,
                Sensor::FanCPUOpt,
            ],
        }),
        "prowsx570ace" => Some(BoardConfig {
            family: Family::Amd500,
            sensors: &[
                Sensor::TempChipset,
                Sensor::TempCPU,
                Sensor::TempMB,
                Sensor::TempVrm,
                Sensor::FanChipset,
            ],
        }),
        "rogcrosshairviiihero" => Some(BoardConfig {
            family: Family::Amd500,
            sensors: &[
                Sensor::TempChipset,
                Sensor::TempCPU,
                Sensor::TempMB,
                Sensor::TempTSensor,
                Sensor::TempVrm,
                Sensor::TempWaterIn,
                Sensor::TempWaterOut,
                Sensor::FanCPUOpt,
                Sensor::FanChipset,
            ],
        }),
        "rogcrosshairviiiherowifi" => Some(BoardConfig {
            family: Family::Amd500,
            sensors: &[
                Sensor::TempChipset,
                Sensor::TempCPU,
                Sensor::TempMB,
                Sensor::TempTSensor,
                Sensor::TempVrm,
                Sensor::TempWaterIn,
                Sensor::TempWaterOut,
                Sensor::FanCPUOpt,
                Sensor::FanChipset,
            ],
        }),
        "rogcrosshairviiiformula" => Some(BoardConfig {
            family: Family::Amd500,
            sensors: &[
                Sensor::TempChipset,
                Sensor::TempCPU,
                Sensor::TempMB,
                Sensor::TempTSensor,
                Sensor::TempVrm,
                Sensor::TempWaterIn,
                Sensor::TempWaterOut,
                Sensor::FanCPUOpt,
                Sensor::FanChipset,
            ],
        }),
        "rogcrosshairx670eextreme" => Some(BoardConfig {
            family: Family::Amd600,
            sensors: &[Sensor::TempWaterIn, Sensor::TempWaterOut, Sensor::FanCPUOpt],
        }),
        "rogcrosshairx670ehero" => Some(BoardConfig {
            family: Family::Amd600,
            sensors: &[Sensor::TempWaterIn, Sensor::TempWaterOut, Sensor::FanCPUOpt],
        }),
        "rogcrosshairx670egene" => Some(BoardConfig {
            family: Family::Amd600,
            sensors: &[Sensor::TempWaterIn, Sensor::TempWaterOut, Sensor::FanCPUOpt],
        }),
        "rogstrixx670eegamingwifi" => Some(BoardConfig {
            family: Family::Amd600,
            sensors: &[Sensor::TempWaterIn, Sensor::TempWaterOut, Sensor::FanCPUOpt],
        }),
        "rogstrixx670efgamingwifi" => Some(BoardConfig {
            family: Family::Amd600,
            sensors: &[Sensor::TempWaterIn, Sensor::TempWaterOut, Sensor::FanCPUOpt],
        }),
        "rogcrosshairviiidarkhero" => Some(BoardConfig {
            family: Family::Amd500,
            sensors: &[
                Sensor::TempChipset,
                Sensor::TempCPU,
                Sensor::TempMB,
                Sensor::TempTSensor,
                Sensor::TempVrm,
                Sensor::TempWaterIn,
                Sensor::TempWaterOut,
                Sensor::FanCPUOpt,
            ],
        }),
        "rogcrosshairviiiimpact" => Some(BoardConfig {
            family: Family::Amd500,
            sensors: &[
                Sensor::TempChipset,
                Sensor::TempCPU,
                Sensor::TempMB,
                Sensor::TempTSensor,
                Sensor::TempVrm,
                Sensor::FanChipset,
            ],
        }),
        "rogstrixb550egaming" => Some(BoardConfig {
            family: Family::Amd500,
            sensors: &[
                Sensor::TempChipset,
                Sensor::TempCPU,
                Sensor::TempMB,
                Sensor::TempTSensor,
                Sensor::TempVrm,
                Sensor::FanCPUOpt,
            ],
        }),
        "rogstrixb550igaming" => Some(BoardConfig {
            family: Family::Amd500,
            sensors: &[
                Sensor::TempChipset,
                Sensor::TempCPU,
                Sensor::TempMB,
                Sensor::TempTSensor,
                Sensor::TempVrm,
                Sensor::FanVrmHS,
            ],
        }),
        "rogstrixx570egaming" => Some(BoardConfig {
            family: Family::Amd500,
            sensors: &[
                Sensor::TempChipset,
                Sensor::TempCPU,
                Sensor::TempMB,
                Sensor::TempTSensor,
                Sensor::TempVrm,
                Sensor::FanChipset,
            ],
        }),
        "rogstrixx570egamingwifiii" => Some(BoardConfig {
            family: Family::Amd500,
            sensors: &[
                Sensor::TempChipset,
                Sensor::TempCPU,
                Sensor::TempMB,
                Sensor::TempTSensor,
                Sensor::TempVrm,
            ],
        }),
        "rogstrixx570fgaming" => Some(BoardConfig {
            family: Family::Amd500,
            sensors: &[
                Sensor::TempChipset,
                Sensor::TempCPU,
                Sensor::TempMB,
                Sensor::TempTSensor,
                Sensor::FanChipset,
            ],
        }),
        "rogstrixx570igaming" => Some(BoardConfig {
            family: Family::Amd500,
            sensors: &[
                Sensor::TempTSensor,
                Sensor::FanVrmHS,
                Sensor::FanChipset,
                Sensor::TempChipset,
                Sensor::TempVrm,
            ],
        }),
        "rogstrixz370ggaming" => Some(BoardConfig {
            family: Family::Intel370,
            sensors: &[
                Sensor::TempChipset,
                Sensor::TempTSensor,
                Sensor::FanCPUOpt,
                Sensor::FanWaterPump,
            ],
        }),
        "rogstrixz390egaming" => Some(BoardConfig {
            family: Family::Intel300,
            sensors: &[
                Sensor::TempVrm,
                Sensor::TempChipset,
                Sensor::TempTSensor,
                Sensor::FanCPUOpt,
            ],
        }),
        "rogstrixz390fgaming" => Some(BoardConfig {
            family: Family::Intel300,
            sensors: &[
                Sensor::TempVrm,
                Sensor::TempChipset,
                Sensor::TempTSensor,
                Sensor::FanCPUOpt,
            ],
        }),
        "rogstrixz390igaming" => Some(BoardConfig {
            family: Family::Intel300,
            sensors: &[Sensor::TempVrm, Sensor::TempChipset, Sensor::TempTSensor],
        }),
        "rogmaximusxiformula" => Some(BoardConfig {
            family: Family::Intel300,
            sensors: &[
                Sensor::TempVrm,
                Sensor::TempChipset,
                Sensor::TempWaterIn,
                Sensor::TempWaterOut,
                Sensor::TempTSensor,
                Sensor::FanCPUOpt,
            ],
        }),
        "rogmaximusxiiformula" => Some(BoardConfig {
            family: Family::Intel400,
            sensors: &[
                Sensor::TempTSensor,
                Sensor::TempVrm,
                Sensor::TempWaterIn,
                Sensor::TempWaterOut,
            ],
        }),
        "rogstrixz690agamingwifid4" => Some(BoardConfig {
            family: Family::Intel600,
            sensors: &[Sensor::TempTSensor, Sensor::TempVrm],
        }),
        "rogstrixz690ggamingwifi" => Some(BoardConfig {
            family: Family::Intel600,
            sensors: &[Sensor::TempTSensor, Sensor::TempVrm],
        }),
        "rogmaximusz690hero" => Some(BoardConfig {
            family: Family::Intel600,
            sensors: &[
                Sensor::TempTSensor,
                Sensor::TempWaterIn,
                Sensor::TempWaterOut,
            ],
        }),
        "rogmaximusz690formula" => Some(BoardConfig {
            family: Family::Intel600,
            sensors: &[
                Sensor::TempTSensor,
                Sensor::TempVrm,
                Sensor::TempWaterIn,
                Sensor::TempWaterOut,
                Sensor::TempWaterBlockIn,
            ],
        }),
        "rogmaximusz690extremeglacial" => Some(BoardConfig {
            family: Family::Intel600,
            sensors: &[
                Sensor::TempVrm,
                Sensor::TempWaterIn,
                Sensor::TempWaterOut,
                Sensor::TempWaterBlockIn,
            ],
        }),
        "rogmaximusz790hero" => Some(BoardConfig {
            family: Family::Intel700,
            sensors: &[
                Sensor::TempVrm,
                Sensor::TempTSensor,
                Sensor::TempWaterIn,
                Sensor::TempWaterOut,
                Sensor::FanCPUOpt,
            ],
        }),
        "rogmaximusz790darkhero" => Some(BoardConfig {
            family: Family::Intel700,
            sensors: &[
                Sensor::TempVrm,
                Sensor::FanCPUOpt,
                Sensor::TempTSensor,
                Sensor::TempWaterIn,
                Sensor::TempWaterOut,
            ],
        }),
        "z170a" => Some(BoardConfig {
            family: Family::Intel100,
            sensors: &[
                Sensor::TempTSensor,
                Sensor::TempChipset,
                Sensor::FanWaterPump,
            ],
        }),
        "z170progaming" => Some(BoardConfig {
            family: Family::Intel100,
            sensors: &[Sensor::TempChipset, Sensor::TempVrm, Sensor::TempTSensor],
        }),
        "primez690a" => Some(BoardConfig {
            family: Family::Intel600,
            sensors: &[Sensor::TempTSensor, Sensor::TempVrm],
        }),
        "rogstrixz790igamingwifi" => Some(BoardConfig {
            family: Family::Intel700,
            sensors: &[Sensor::TempTSensor, Sensor::TempTSensor2],
        }),
        "rogstrixz790egamingwifi" => Some(BoardConfig {
            family: Family::Intel700,
            sensors: &[Sensor::TempWaterIn],
        }),
        "rogstrixz790egamingwifiii" => Some(BoardConfig {
            family: Family::Intel700,
            sensors: &[Sensor::TempTSensor, Sensor::TempVrm, Sensor::FanCPUOpt],
        }),
        "rogstrixz890egamingwifi" => Some(BoardConfig {
            family: Family::Intel800,
            sensors: &[Sensor::TempTSensor, Sensor::TempVrm],
        }),
        "rogmaximusz790formula" => Some(BoardConfig {
            family: Family::Intel700,
            sensors: &[Sensor::TempWaterIn, Sensor::TempWaterOut],
        }),
        "rogmaximusxiiherowifi" => Some(BoardConfig {
            family: Family::Intel400,
            sensors: &[
                Sensor::TempTSensor,
                Sensor::TempChipset,
                Sensor::TempVrm,
                Sensor::TempWaterIn,
                Sensor::TempWaterOut,
                Sensor::FanCPUOpt,
            ],
        }),
        "rogstrixx870igamingwifi" => Some(BoardConfig {
            family: Family::Amd800,
            sensors: &[
                Sensor::TempCPU,
                Sensor::TempCPUPackage,
                Sensor::TempMB,
                Sensor::TempVrm,
            ],
        }),
        _ => None,
    }
}

fn sensor_spec(family: Family, sensor: Sensor) -> Option<SensorSpec> {
    let temperature = |key, name, register, blank| SensorSpec {
        key,
        name,
        kind: Kind::Temperature,
        register,
        size: 1,
        blank,
    };
    let fan = |key, name, register| SensorSpec {
        key,
        name,
        kind: Kind::Fan,
        register,
        size: 2,
        blank: None,
    };

    match (family, sensor) {
        (Family::Amd400, Sensor::TempChipset) | (Family::Amd500, Sensor::TempChipset) => {
            Some(temperature("chipset", "Chipset", 0x003A, None))
        }
        (Family::Amd400, Sensor::TempCPU) | (Family::Amd500, Sensor::TempCPU) => {
            Some(temperature("cpu", "CPU", 0x003B, None))
        }
        (Family::Amd400, Sensor::TempMB) | (Family::Amd500, Sensor::TempMB) => {
            Some(temperature("motherboard", "Motherboard", 0x003C, None))
        }
        (Family::Amd400, Sensor::TempTSensor) | (Family::Amd500, Sensor::TempTSensor) => {
            Some(temperature("t-sensor", "T Sensor", 0x003D, Some(-40)))
        }
        (Family::Amd400, Sensor::TempVrm) | (Family::Amd500, Sensor::TempVrm) => {
            Some(temperature("vrm", "VRM", 0x003E, None))
        }
        (Family::Amd400, Sensor::FanCPUOpt) => Some(fan("cpu-opt-fan", "CPU Optional Fan", 0x00BC)),
        (Family::Amd500, Sensor::FanCPUOpt)
        | (Family::Amd600, Sensor::FanCPUOpt)
        | (Family::Amd800, Sensor::FanCPUOpt) => {
            Some(fan("cpu-opt-fan", "CPU Optional Fan", 0x00B0))
        }
        (Family::Amd400, Sensor::FanVrmHS) | (Family::Amd500, Sensor::FanVrmHS) => {
            Some(fan("vrm-heatsink-fan", "VRM Heat Sink Fan", 0x00B2))
        }
        (Family::Amd500, Sensor::FanChipset) => Some(fan("chipset-fan", "Chipset Fan", 0x00B4)),
        (Family::Amd400, Sensor::TempWaterIn) => {
            Some(temperature("water-in", "Water In", 0x010D, Some(-40)))
        }
        (Family::Amd400, Sensor::TempWaterOut) => {
            Some(temperature("water-out", "Water Out", 0x010B, Some(-40)))
        }
        (Family::Amd500, Sensor::TempWaterIn) | (Family::Amd600, Sensor::TempWaterIn) => {
            Some(temperature("water-in", "Water In", 0x0100, Some(-40)))
        }
        (Family::Amd500, Sensor::TempWaterOut) | (Family::Amd600, Sensor::TempWaterOut) => {
            Some(temperature("water-out", "Water Out", 0x0101, Some(-40)))
        }
        (Family::Amd800, Sensor::TempCPU) => Some(temperature("cpu", "CPU", 0x0030, None)),
        (Family::Amd800, Sensor::TempCPUPackage) => {
            Some(temperature("cpu-package", "CPU Package", 0x0031, None))
        }
        (Family::Amd800, Sensor::TempMB) => {
            Some(temperature("motherboard", "Motherboard", 0x0032, None))
        }
        (Family::Amd800, Sensor::TempVrm) => Some(temperature("vrm", "VRM", 0x0033, None)),
        (Family::Amd800, Sensor::TempTSensorAlt) => {
            Some(temperature("t-sensor", "T Sensor", 0x0035, Some(-40)))
        }
        (Family::Amd800, Sensor::TempTSensor) => {
            Some(temperature("t-sensor", "T Sensor", 0x0036, Some(-40)))
        }
        (Family::Intel100, Sensor::TempChipset)
        | (Family::Intel300, Sensor::TempChipset)
        | (Family::Intel370, Sensor::TempChipset)
        | (Family::Intel400, Sensor::TempChipset) => {
            Some(temperature("chipset", "Chipset", 0x003A, None))
        }
        (Family::Intel100, Sensor::TempVrm)
        | (Family::Intel300, Sensor::TempVrm)
        | (Family::Intel400, Sensor::TempVrm)
        | (Family::Intel600, Sensor::TempVrm) => Some(temperature("vrm", "VRM", 0x003E, None)),
        (Family::Intel100, Sensor::TempTSensor)
        | (Family::Intel300, Sensor::TempTSensor)
        | (Family::Intel370, Sensor::TempTSensor)
        | (Family::Intel400, Sensor::TempTSensor)
        | (Family::Intel600, Sensor::TempTSensor) => {
            Some(temperature("t-sensor", "T Sensor", 0x003D, Some(-40)))
        }
        (Family::Intel300, Sensor::TempWaterIn)
        | (Family::Intel400, Sensor::TempWaterIn)
        | (Family::Intel600, Sensor::TempWaterIn) => {
            Some(temperature("water-in", "Water In", 0x0100, Some(-40)))
        }
        (Family::Intel300, Sensor::TempWaterOut)
        | (Family::Intel400, Sensor::TempWaterOut)
        | (Family::Intel600, Sensor::TempWaterOut) => {
            Some(temperature("water-out", "Water Out", 0x0101, Some(-40)))
        }
        (Family::Intel600, Sensor::TempWaterBlockIn) => Some(temperature(
            "water-block-in",
            "Water Block In",
            0x0102,
            Some(-40),
        )),
        (Family::Intel300, Sensor::FanCPUOpt) | (Family::Intel400, Sensor::FanCPUOpt) => {
            Some(fan("cpu-opt-fan", "CPU Optional Fan", 0x00B0))
        }
        (Family::Intel370, Sensor::FanCPUOpt) => {
            Some(fan("cpu-opt-fan", "CPU Optional Fan", 0x00BC))
        }
        (Family::Intel100, Sensor::FanWaterPump) => Some(fan("water-pump", "Water Pump", 0x00BC)),
        (Family::Intel370, Sensor::FanWaterPump) => Some(fan("water-pump", "Water Pump", 0x00BE)),
        (Family::Intel700, Sensor::TempVrm) | (Family::Intel800, Sensor::TempVrm) => {
            Some(temperature("vrm", "VRM", 0x0033, None))
        }
        (Family::Intel700, Sensor::FanCPUOpt) => {
            Some(fan("cpu-opt-fan", "CPU Optional Fan", 0x00B0))
        }
        (Family::Intel700, Sensor::TempTSensor) => {
            Some(temperature("t-sensor", "T Sensor", 0x0109, Some(-40)))
        }
        (Family::Intel700, Sensor::TempTSensor2) => {
            Some(temperature("t-sensor-2", "T Sensor 2", 0x0105, Some(-40)))
        }
        (Family::Intel700, Sensor::TempWaterIn) => {
            Some(temperature("water-in", "Water In", 0x0100, Some(-40)))
        }
        (Family::Intel700, Sensor::TempWaterOut) => {
            Some(temperature("water-out", "Water Out", 0x0101, Some(-40)))
        }
        (Family::Intel800, Sensor::TempTSensor) => {
            Some(temperature("t-sensor", "T Sensor", 0x010C, Some(-40)))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reviewed_board_table_is_exact_and_unknown_boards_are_not_probed() {
        let known = board_config("rogcrosshairx870ehero").unwrap();
        assert!(matches!(known.family, Family::Amd800));
        assert!(
            known
                .sensors
                .iter()
                .any(|sensor| matches!(sensor, Sensor::TempVrm))
        );
        assert!(board_config("unknownboard").is_none());
    }

    #[test]
    fn reviewed_family_registers_keep_blank_temperature_semantics() {
        let spec = sensor_spec(Family::Intel700, Sensor::TempTSensor).unwrap();
        assert_eq!(spec.register, 0x0109);
        assert_eq!(spec.blank, Some(-40));
        let fan = sensor_spec(Family::Amd500, Sensor::FanChipset).unwrap();
        assert_eq!(fan.register, 0x00B4);
        assert_eq!(fan.size, 2);
    }
}
