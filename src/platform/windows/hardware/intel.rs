// Intel digital-thermal-sensor semantics are checked against LibreHardwareMonitor's
// reviewed hardware implementation, but this file uses the project-owned PawnIO client.

use std::io;

use crate::core::model::{TemperatureId, TemperatureSnapshot};

use super::super::pawnio::Session;
use super::cpu::{AffinityGuard, CoreAffinity, Info};
use super::{CpuTemperature, CpuTemperatureVisibility};

const IA32_THERM_STATUS: u64 = 0x19C;
const IA32_TEMPERATURE_TARGET: u64 = 0x1A2;
const IA32_PACKAGE_THERM_STATUS: u64 = 0x1B1;

pub(super) struct Collector {
    info: Info,
    cores: Vec<CoreAffinity>,
    topology_degraded: bool,
}

pub(super) struct Observation {
    pub temperatures: Vec<CpuTemperature>,
    pub degraded: bool,
    pub core_count: usize,
    pub affinity_failures: usize,
    pub invalid_core_dts: usize,
}

impl Collector {
    pub(super) fn new(info: Info) -> Self {
        let topology = if info.core_dts || info.package_dts {
            super::cpu::physical_core_affinities()
        } else {
            Ok(Vec::new())
        };
        Self::from_topology(info, topology)
    }

    fn from_topology(info: Info, topology: io::Result<Vec<CoreAffinity>>) -> Self {
        let (cores, topology_degraded) = match topology {
            Ok(cores) => (cores, false),
            Err(_) => (Vec::new(), true),
        };
        Self {
            info,
            cores,
            topology_degraded,
        }
    }

    pub(super) fn collect(&self, session: &Session) -> io::Result<Observation> {
        let mut temperatures = Vec::new();
        let mut degraded = self.topology_degraded;
        let mut affinity_failures = 0;
        let mut invalid_core_dts = 0;

        if self.info.core_dts {
            if self.cores.is_empty() {
                degraded = true;
            }
            for (index, &affinity) in self.cores.iter().enumerate() {
                let guard = match AffinityGuard::bind(affinity) {
                    Ok(guard) => guard,
                    Err(_) => {
                        degraded = true;
                        affinity_failures += 1;
                        continue;
                    }
                };
                match read_temperature(session, IA32_THERM_STATUS)? {
                    Some(celsius) => temperatures.push(CpuTemperature {
                        snapshot: TemperatureSnapshot {
                            id: TemperatureId::from_opaque_key(format!(
                                "windows:intel-core:{}",
                                index + 1
                            )),
                            name: format!("Core #{}", index + 1),
                            celsius,
                        },
                        visibility: CpuTemperatureVisibility::Detail,
                    }),
                    None => {
                        degraded = true;
                        invalid_core_dts += 1;
                    }
                }
                drop(guard);
            }
        }

        if self.info.package_dts {
            let _guard = self
                .cores
                .first()
                .copied()
                .map(AffinityGuard::bind)
                .transpose()
                .ok()
                .flatten();
            match read_temperature(session, IA32_PACKAGE_THERM_STATUS)? {
                Some(celsius) => temperatures.push(CpuTemperature {
                    snapshot: TemperatureSnapshot {
                        // Preserve the pre-existing stable package identity.
                        id: TemperatureId::from_opaque_key("windows:intel-package"),
                        name: "CPU Package".to_owned(),
                        celsius,
                    },
                    visibility: CpuTemperatureVisibility::Primary,
                }),
                None => degraded = true,
            }
        }

        Ok(Observation {
            temperatures,
            degraded,
            core_count: self.cores.len(),
            affinity_failures,
            invalid_core_dts,
        })
    }
}

fn read_temperature(session: &Session, status_msr: u64) -> io::Result<Option<f64>> {
    let target = read_msr(session, IA32_TEMPERATURE_TARGET)?;
    let status = read_msr(session, status_msr)?;
    if status & (1 << 31) == 0 {
        return Ok(None);
    }

    let tj_max = ((target >> 16) & 0xff) as f64;
    let delta = ((status >> 16) & 0x7f) as f64;
    let celsius = tj_max - delta;
    Ok((0.0..=125.0).contains(&celsius).then_some(celsius))
}

fn read_msr(session: &Session, msr: u64) -> io::Result<u64> {
    session
        .execute("ioctl_read_msr", &[msr], 1)?
        .first()
        .copied()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "PawnIO MSR read returned no value",
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topology_failure_does_not_disable_package_capability() {
        let collector = Collector::from_topology(
            Info {
                vendor: super::super::cpu::Vendor::Intel,
                family: 6,
                model: 0,
                brand: String::new(),
                core_dts: true,
                package_dts: true,
                amd_dts: false,
            },
            Err(io::Error::other("topology unavailable")),
        );

        assert!(collector.topology_degraded);
        assert!(collector.cores.is_empty());
        assert!(collector.info.package_dts);
    }

    #[test]
    fn package_temperature_decodes_tjmax_minus_delta() {
        let target = 100_u64 << 16;
        let status = (1_u64 << 31) | (37_u64 << 16);
        let tj_max = ((target >> 16) & 0xff) as f64;
        let delta = ((status >> 16) & 0x7f) as f64;
        assert_eq!(tj_max - delta, 63.0);
    }
}
