// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// https://mozilla.org/MPL/2.0/.
// Copyright (C) LibreHardwareMonitor and Contributors.
// AMD temperature decoding follows LibreHardwareMonitor Amd0FCpu/Amd10Cpu/Amd17Cpu
// at reviewed commit 677a3a56abde9adff5abdb42db3a7638c9137572.

use std::io;

use crate::core::model::{TemperatureId, TemperatureSnapshot};

use super::super::pawnio::Session;
use super::cpu::Info;
use super::sync::lock_named;
use super::{CpuTemperature, CpuTemperatureVisibility};

const PCI_MUTEX: &str = "Global\\Access_PCI";
const PCI_WAIT_MS: u32 = 100;
const REPORTED_TEMPERATURE_CONTROL_REGISTER: u64 = 0xA4;
const SMU_REPORTED_TEMP_CTRL_OFFSET: u64 = 0xD8200CA4;
const F17H_M01H_THM_TCON_CUR_TMP: u64 = 0x0005_9800;
const F17H_M70H_CCD1_TEMP: u64 = 0x0005_9954;
const F17H_M61H_CCD1_TEMP: u64 = 0x0005_9B08;
const F17H_TEMP_RANGE_SEL_MASK: u64 = 0x0008_0000;
const F17H_TEMP_TJ_SEL_MASK: u64 = 0x0003_0000;

pub(super) struct Collector {
    info: Info,
    kind: Kind,
    family0f_core_count: usize,
}

#[derive(Clone, Copy)]
enum Kind {
    Family0F,
    Family10To16,
    Family17Plus,
}

pub(super) struct Observation {
    pub temperatures: Vec<CpuTemperature>,
    pub degraded: bool,
}

impl Collector {
    pub(super) fn new(info: Info) -> io::Result<Option<Self>> {
        let kind = match info.family {
            0x0F => Kind::Family0F,
            0x10..=0x16 => Kind::Family10To16,
            0x17..=0x1A => Kind::Family17Plus,
            _ => return Ok(None),
        };
        let family0f_core_count = if matches!(kind, Kind::Family0F) && info.amd_dts {
            super::cpu::physical_core_affinities()?.len().clamp(1, 2)
        } else {
            0
        };
        Ok(Some(Self {
            info,
            kind,
            family0f_core_count,
        }))
    }

    pub(super) fn module(&self) -> &'static [u8] {
        match self.kind {
            Kind::Family0F => include_bytes!(env!("CCLOVER_PAWNIO_AMD_FAMILY_0F_BIN")),
            Kind::Family10To16 => include_bytes!(env!("CCLOVER_PAWNIO_AMD_FAMILY_10_BIN")),
            Kind::Family17Plus => include_bytes!(env!("CCLOVER_PAWNIO_AMD_FAMILY_17_BIN")),
        }
    }

    pub(super) fn collect(&self, session: &Session) -> io::Result<Observation> {
        let _guard = lock_named(PCI_MUTEX, PCI_WAIT_MS, "PCI bus")?;
        match self.kind {
            Kind::Family0F => self.collect_family0f(session),
            Kind::Family10To16 => self.collect_family10(session),
            Kind::Family17Plus => self.collect_family17(session),
        }
    }

    fn collect_family0f(&self, session: &Session) -> io::Result<Observation> {
        let mut temperatures = Vec::new();
        let mut degraded = false;
        if !self.info.amd_dts {
            return Ok(Observation {
                temperatures,
                degraded,
            });
        }

        let mut offset = -49.0;
        if self.info.model >= 0x69 && !matches!(self.info.model, 0xC1 | 0x6C | 0x7C) {
            offset += 21.0;
        }
        for core in 0..self.family0f_core_count {
            match execute1(session, "ioctl_get_thermtrip", &[0, core as u64]) {
                Ok(raw) => {
                    let celsius = f64::from(((raw >> 16) & 0xFF) as u8) + offset;
                    temperatures.push(CpuTemperature {
                        snapshot: TemperatureSnapshot {
                            id: TemperatureId::from_opaque_key(format!(
                                "windows:amd-family0f-core:{}",
                                core + 1
                            )),
                            name: format!("Core #{}", core + 1),
                            celsius,
                        },
                        visibility: CpuTemperatureVisibility::Detail,
                    });
                }
                Err(error) if error.raw_os_error().is_none() => degraded = true,
                Err(error) => return Err(error),
            }
        }
        Ok(Observation {
            temperatures,
            degraded,
        })
    }

    fn collect_family10(&self, session: &Session) -> io::Result<Observation> {
        let raw = if self.info.family == 0x15 && matches!(self.info.model & 0xF0, 0x60 | 0x70) {
            execute1(session, "ioctl_read_smu", &[SMU_REPORTED_TEMP_CTRL_OFFSET])?
        } else {
            execute1(
                session,
                "ioctl_read_miscctl",
                &[0, REPORTED_TEMPERATURE_CONTROL_REGISTER],
            )?
        };
        let celsius = decode_family10_temperature(self.info.family, self.info.model, raw);
        Ok(Observation {
            temperatures: vec![CpuTemperature {
                snapshot: TemperatureSnapshot {
                    id: TemperatureId::from_opaque_key("windows:amd-cores"),
                    name: "CPU Cores".to_owned(),
                    celsius,
                },
                visibility: CpuTemperatureVisibility::Primary,
            }],
            degraded: false,
        })
    }

    fn collect_family17(&self, session: &Session) -> io::Result<Observation> {
        let raw = execute1(session, "ioctl_read_smn", &[F17H_M01H_THM_TCON_CUR_TMP])?;
        let mut t = f64::from(((raw >> 21) * 125) as u32) * 0.001;
        if raw & F17H_TEMP_RANGE_SEL_MASK != 0
            || raw & F17H_TEMP_TJ_SEL_MASK == F17H_TEMP_TJ_SEL_MASK
        {
            t -= 49.0;
        }

        let die_offset = brand_temperature_offset(&self.info.brand);
        let mut temperatures = Vec::new();
        if die_offset < 0.0 {
            temperatures.push(CpuTemperature {
                snapshot: TemperatureSnapshot {
                    id: TemperatureId::from_opaque_key("windows:amd-tctl"),
                    name: "Core (Tctl)".to_owned(),
                    celsius: t,
                },
                visibility: CpuTemperatureVisibility::Primary,
            });
            temperatures.push(CpuTemperature {
                snapshot: TemperatureSnapshot {
                    id: TemperatureId::from_opaque_key("windows:amd-tdie"),
                    name: "Core (Tdie)".to_owned(),
                    celsius: t + die_offset,
                },
                visibility: CpuTemperatureVisibility::Primary,
            });
        } else {
            temperatures.push(CpuTemperature {
                snapshot: TemperatureSnapshot {
                    id: TemperatureId::from_opaque_key("windows:amd-tctl-tdie"),
                    name: "Core (Tctl/Tdie)".to_owned(),
                    celsius: t,
                },
                visibility: CpuTemperatureVisibility::Primary,
            });
        }

        let mut degraded = false;
        if supports_ccd_temperatures(self.info.model) {
            let base = if matches!(self.info.model, 0x61 | 0x44) {
                F17H_M61H_CCD1_TEMP
            } else {
                F17H_M70H_CCD1_TEMP
            };
            for ccd in 0..8_u64 {
                match execute1(session, "ioctl_read_smn", &[base + ccd * 4]) {
                    Ok(raw) => {
                        let raw = raw & 0xFFF;
                        let celsius = (raw as f64 * 125.0 - 305_000.0) * 0.001;
                        if raw > 0 && celsius < 125.0 {
                            temperatures.push(CpuTemperature {
                                snapshot: TemperatureSnapshot {
                                    id: TemperatureId::from_opaque_key(format!(
                                        "windows:amd-ccd:{}",
                                        ccd + 1
                                    )),
                                    name: format!("CCD{} (Tdie)", ccd + 1),
                                    celsius,
                                },
                                visibility: CpuTemperatureVisibility::Detail,
                            });
                        }
                    }
                    Err(error) if error.raw_os_error().is_none() => degraded = true,
                    Err(error) => return Err(error),
                }
            }
        }

        Ok(Observation {
            temperatures,
            degraded,
        })
    }
}

fn execute1(session: &Session, name: &str, input: &[u64]) -> io::Result<u64> {
    session
        .execute(name, input, 1)?
        .first()
        .copied()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "PawnIO returned no value"))
}

fn decode_family10_temperature(family: u32, model: u32, value: u64) -> f64 {
    if matches!(family, 0x15 | 0x16) && value & 0x30000 == 0x3000 {
        if family == 0x15 && model & 0xF0 == 0 {
            f64::from(((value >> 21) & 0x7FC) as u32) / 8.0 - 49.0
        } else {
            f64::from(((value >> 21) & 0x7FF) as u32) / 8.0 - 49.0
        }
    } else {
        f64::from(((value >> 21) & 0x7FF) as u32) / 8.0
    }
}

fn brand_temperature_offset(brand: &str) -> f64 {
    if ["1600X", "1700X", "1800X"]
        .iter()
        .any(|needle| brand.contains(needle))
    {
        -20.0
    } else if brand.contains("Threadripper 19") || brand.contains("Threadripper 29") {
        -27.0
    } else if brand.contains("2700X") {
        -10.0
    } else {
        0.0
    }
}

fn supports_ccd_temperatures(model: u32) -> bool {
    matches!(model, 0x31 | 0x71 | 0x21 | 0x61 | 0x44)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zen_brand_offsets_match_reviewed_lhm_table() {
        assert_eq!(brand_temperature_offset("AMD Ryzen 7 1800X"), -20.0);
        assert_eq!(
            brand_temperature_offset("AMD Ryzen Threadripper 2950X"),
            -27.0
        );
        assert_eq!(brand_temperature_offset("AMD Ryzen 7 2700X"), -10.0);
        assert_eq!(brand_temperature_offset("AMD Ryzen 9 7900X"), 0.0);
    }

    #[test]
    fn ccd_capability_is_model_based_not_channel_guessing() {
        assert!(supports_ccd_temperatures(0x71));
        assert!(supports_ccd_temperatures(0x61));
        assert!(!supports_ccd_temperatures(0x01));
    }

    #[test]
    fn family10_temperature_uses_reported_temperature_field() {
        let raw = 512_u64 << 21;
        assert_eq!(decode_family10_temperature(0x10, 0, raw), 64.0);
    }
}
