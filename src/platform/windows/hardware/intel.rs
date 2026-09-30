use std::io;

use crate::core::model::{TemperatureId, TemperatureSnapshot};

use super::super::pawnio::Session;
use super::{CpuTemperature, CpuTemperatureVisibility, coretemp, cpu::Info};

pub(super) struct Collector {
    upstream: coretemp::Collector,
}

pub(super) struct Observation {
    pub temperatures: Vec<CpuTemperature>,
    pub degraded: bool,
    pub core_count: usize,
    pub affinity_failures: usize,
    pub invalid_core_dts: usize,
}

impl Collector {
    pub(super) fn new(info: Info) -> io::Result<Self> {
        coretemp::Collector::new(&info).map(|upstream| Self { upstream })
    }

    pub(super) fn collect(&mut self, session: &Session) -> io::Result<Observation> {
        let observation = self.upstream.collect(session)?;
        let mut temperatures = Vec::with_capacity(
            observation.core_temperatures.len()
                + usize::from(observation.package_temperature.is_some()),
        );

        for (index, celsius) in observation.core_temperatures {
            temperatures.push(CpuTemperature {
                snapshot: TemperatureSnapshot {
                    id: TemperatureId::from_opaque_key(format!("windows:intel-core:{}", index + 1)),
                    name: format!("Core #{}", index + 1),
                    celsius,
                },
                visibility: CpuTemperatureVisibility::Detail,
            });
        }

        if let Some(celsius) = observation.package_temperature {
            temperatures.push(CpuTemperature {
                snapshot: TemperatureSnapshot {
                    id: TemperatureId::from_opaque_key("windows:intel-package"),
                    name: "CPU Package".to_owned(),
                    celsius,
                },
                visibility: CpuTemperatureVisibility::Primary,
            });
        }

        Ok(Observation {
            temperatures,
            degraded: observation.degraded,
            core_count: observation.core_count,
            affinity_failures: observation.affinity_failures,
            invalid_core_dts: observation.invalid_core_dts,
        })
    }
}
