// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// https://mozilla.org/MPL/2.0/.
// Copyright (C) LibreHardwareMonitor and Contributors.
// Legacy Family 10h decoding follows LibreHardwareMonitor at reviewed commit
// 677a3a56abde9adff5abdb42db3a7638c9137572. Family 0F delegates to Linux k8temp; 11h+ to k10temp.

use std::io;

use crate::core::model::{TemperatureId, TemperatureSnapshot};

use super::super::pawnio::Session;
use super::cpu::Info;
use super::sync::lock_named;
use super::{CpuTemperature, CpuTemperatureVisibility};

const PCI_MUTEX: &str = "Global\\Access_PCI";
const PCI_WAIT_MS: u32 = 100;
const REPORTED_TEMPERATURE_CONTROL_REGISTER: u64 = 0xA4;
pub(super) struct Collector {
    info: Info,
    kind: Kind,
    k8temp: Option<super::k8temp::Collector>,
    k10temp: Option<super::k10temp::Collector>,
}

#[derive(Clone, Copy)]
enum Kind {
    Family0F,
    Family10,
    Family11To16,
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
            0x10 => Kind::Family10,
            0x11..=0x16 => Kind::Family11To16,
            0x17..=0x1A => Kind::Family17Plus,
            _ => return Ok(None),
        };
        Ok(Some(Self {
            info,
            kind,
            k8temp: None,
            k10temp: None,
        }))
    }

    pub(super) fn module(&self) -> &'static [u8] {
        match self.kind {
            Kind::Family0F => include_bytes!(env!("CCLOVER_PAWNIO_AMD_FAMILY_0F_BIN")),
            Kind::Family10 | Kind::Family11To16 => {
                include_bytes!(env!("CCLOVER_PAWNIO_AMD_FAMILY_10_BIN"))
            }
            Kind::Family17Plus => include_bytes!(env!("CCLOVER_PAWNIO_AMD_FAMILY_17_BIN")),
        }
    }

    pub(super) fn collect(&mut self, session: &Session) -> io::Result<Observation> {
        let _guard = lock_named(PCI_MUTEX, PCI_WAIT_MS, "PCI bus")?;
        match self.kind {
            Kind::Family0F => self.collect_family0f(session),
            Kind::Family10 => self.collect_family10(session),
            Kind::Family11To16 => self.collect_k10temp(session, false),
            Kind::Family17Plus => self.collect_k10temp(session, true),
        }
    }

    fn collect_family0f(&mut self, session: &Session) -> io::Result<Observation> {
        if self.k8temp.is_none() {
            self.k8temp = Some(super::k8temp::Collector::new(&self.info, session)?);
        }
        let channels = self
            .k8temp
            .as_mut()
            .expect("k8temp initialized above")
            .read_channels(session)?;
        let mut temperatures = Vec::new();
        for (channel, celsius) in channels.into_iter().enumerate() {
            let Some(celsius) = celsius else {
                continue;
            };
            let core = channel / 2 + 1;
            temperatures.push(CpuTemperature {
                snapshot: TemperatureSnapshot {
                    id: TemperatureId::from_opaque_key(format!("windows:amd-family0f-core:{core}")),
                    name: format!("Core #{core}"),
                    celsius,
                },
                visibility: CpuTemperatureVisibility::Detail,
            });
        }
        Ok(Observation {
            temperatures,
            degraded: false,
        })
    }

    fn collect_family10(&self, session: &Session) -> io::Result<Observation> {
        let raw = execute1(
            session,
            "ioctl_read_miscctl",
            &[0, REPORTED_TEMPERATURE_CONTROL_REGISTER],
        )?;
        let celsius = decode_family10_temperature(raw);
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

    fn collect_k10temp(&mut self, session: &Session, zen: bool) -> io::Result<Observation> {
        if self.k10temp.is_none() {
            self.k10temp = Some(super::k10temp::Collector::new(&self.info, session)?);
        }
        let channels = self
            .k10temp
            .as_mut()
            .expect("k10temp initialized above")
            .read_channels(session)?;
        let has_tdie = channels.get(1).is_some_and(Option::is_some);
        let mut temperatures = Vec::new();
        for (channel, celsius) in channels.into_iter().enumerate() {
            let Some(celsius) = celsius else {
                continue;
            };
            if !zen {
                temperatures.push(CpuTemperature {
                    snapshot: TemperatureSnapshot {
                        id: TemperatureId::from_opaque_key("windows:amd-cores"),
                        name: "CPU Cores".to_owned(),
                        celsius,
                    },
                    visibility: CpuTemperatureVisibility::Primary,
                });
                break;
            }
            let (id, name, visibility) = match channel {
                0 if has_tdie => (
                    "windows:amd-tctl".to_owned(),
                    "Core (Tctl)".to_owned(),
                    CpuTemperatureVisibility::Primary,
                ),
                0 => (
                    "windows:amd-tctl-tdie".to_owned(),
                    "Core (Tctl/Tdie)".to_owned(),
                    CpuTemperatureVisibility::Primary,
                ),
                1 => (
                    "windows:amd-tdie".to_owned(),
                    "Core (Tdie)".to_owned(),
                    CpuTemperatureVisibility::Primary,
                ),
                ccd => (
                    format!("windows:amd-ccd:{}", ccd - 1),
                    format!("CCD{} (Tdie)", ccd - 1),
                    CpuTemperatureVisibility::Detail,
                ),
            };
            temperatures.push(CpuTemperature {
                snapshot: TemperatureSnapshot {
                    id: TemperatureId::from_opaque_key(id),
                    name,
                    celsius,
                },
                visibility,
            });
        }
        Ok(Observation {
            temperatures,
            degraded: false,
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

fn decode_family10_temperature(value: u64) -> f64 {
    f64::from(((value >> 21) & 0x7FF) as u32) / 8.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family10_temperature_uses_reported_temperature_field() {
        let raw = 512_u64 << 21;
        assert_eq!(decode_family10_temperature(raw), 64.0);
    }
}
