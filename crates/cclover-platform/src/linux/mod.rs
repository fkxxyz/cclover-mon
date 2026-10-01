mod cpu;
mod diagnostics;
mod disk;
mod ebpf_io;
mod fan;
mod gpu;
mod memory;
mod native;
mod network;
mod nvidia;
mod process;
mod temperature;
#[cfg(test)]
mod test_support;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::probe::ProbeSample;
use crate::{ProbeKind, ProbeReport};
use cclover_core::Collector as CoreCollector;
use cclover_core::devlog;
use cclover_core::model::{Collection, GpuSnapshot, RawSnapshot, TemperatureSnapshot};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct PhysicalDeviceId(PathBuf);

impl PhysicalDeviceId {
    fn from_path(path: &Path) -> Option<Self> {
        std::fs::canonicalize(path).ok().map(Self)
    }
}

pub struct Backend {
    processes: process::Collector,
    temperatures: temperature::Collector,
    fans: fan::Collector,
    nvidia: nvidia::Collector,
    ebpf_io: ebpf_io::Collector,
    active_processes: HashSet<cclover_core::model::ProcessInstanceId>,
}

impl Backend {
    pub fn new() -> Self {
        Self {
            processes: process::Collector::new(),
            temperatures: temperature::Collector::new(),
            fans: fan::Collector::new(),
            nvidia: nvidia::Collector::new(),
            ebpf_io: ebpf_io::Collector::new(),
            active_processes: HashSet::new(),
        }
    }

    pub fn probe(&mut self, kind: ProbeKind) -> ProbeReport {
        let mut notes = Vec::new();
        let mut sample = self.collect_probe_once(kind, Some(&mut notes));

        if kind.needs_probe_follow_up() && sample.is_observable() {
            std::thread::sleep(cclover_core::SAMPLE_INTERVAL);
            sample = self.collect_probe_once(kind, Some(&mut notes));
        }

        sample.into_report(notes)
    }

    pub fn collect_for_perf(&mut self, kind: ProbeKind) {
        std::hint::black_box(self.collect_probe_once(kind, None));
    }

    fn collect_probe_once(
        &mut self,
        kind: ProbeKind,
        notes: Option<&mut Vec<String>>,
    ) -> ProbeSample {
        match kind {
            ProbeKind::Cpu => ProbeSample::Cpu(cpu::collect(notes)),
            ProbeKind::Memory => ProbeSample::Memory(memory::collect(notes)),
            ProbeKind::Processes => ProbeSample::Processes(self.processes.collect(notes)),
            ProbeKind::Network => ProbeSample::Network(network::collect(notes)),
            ProbeKind::NetworkAttribution => ProbeSample::NetworkAttribution(
                self.ebpf_io
                    .collect_network(None, notes)
                    .map(|result| result.rows),
            ),
            ProbeKind::Disk => ProbeSample::Disk(disk::collect(notes)),
            ProbeKind::DiskAttribution => ProbeSample::DiskAttribution(
                self.ebpf_io
                    .collect_disk(None, notes)
                    .map(|result| result.rows),
            ),
            ProbeKind::Temperatures => {
                let (_, temperatures) = self.collect_gpu_temperatures(notes);
                ProbeSample::Temperatures(temperatures)
            }
            ProbeKind::Fans => ProbeSample::Fans(self.fans.collect(Instant::now(), notes)),
            ProbeKind::Gpu => {
                let (gpus, _) = self.collect_gpu_temperatures(notes);
                ProbeSample::Gpu(gpus)
            }
        }
    }

    fn collect_gpu_temperatures(
        &mut self,
        mut notes: Option<&mut Vec<String>>,
    ) -> (
        Collection<Vec<GpuSnapshot>>,
        Collection<Vec<TemperatureSnapshot>>,
    ) {
        let temperatures = devlog::timed("collector.temperatures", || {
            self.temperatures
                .collect_observations(Instant::now(), notes.as_deref_mut())
        });
        let gpus = devlog::timed("collector.gpu", || {
            let nvml = self.nvidia.gpus(notes.as_deref_mut());
            gpu::collect_observations(nvml, notes)
        });
        reconcile_gpu_temperatures(gpus, temperatures)
    }
}

fn reconcile_gpu_temperatures(
    mut gpus: Collection<Vec<gpu::Observation>>,
    temperatures: Collection<Vec<temperature::Observation>>,
) -> (
    Collection<Vec<GpuSnapshot>>,
    Collection<Vec<TemperatureSnapshot>>,
) {
    let mut temperatures = match temperatures {
        Collection::Available(values) => Collection::Available(reconcile_temperature_values(
            collection_value_mut(&mut gpus),
            values,
        )),
        Collection::Degraded(values) => Collection::Degraded(reconcile_temperature_values(
            collection_value_mut(&mut gpus),
            values,
        )),
        Collection::Unavailable(reason) => Collection::Unavailable(reason),
    };
    if let Some(values) = collection_value_mut(&mut temperatures) {
        temperature::normalize_display_names(values);
    }
    let gpus = gpus.map(|values| values.into_iter().map(|value| value.snapshot).collect());
    (gpus, temperatures)
}

fn collection_value_mut<T>(collection: &mut Collection<T>) -> Option<&mut T> {
    match collection {
        Collection::Available(value) | Collection::Degraded(value) => Some(value),
        Collection::Unavailable(_) => None,
    }
}

fn reconcile_temperature_values(
    gpus: Option<&mut Vec<gpu::Observation>>,
    temperatures: Vec<temperature::Observation>,
) -> Vec<TemperatureSnapshot> {
    let Some(gpus) = gpus else {
        return temperatures
            .into_iter()
            .map(|value| value.snapshot)
            .collect();
    };

    temperatures
        .into_iter()
        .filter_map(|temperature| {
            let Some(device) = temperature.physical_device.as_ref() else {
                return Some(temperature.snapshot);
            };
            let Some(gpu) = gpus
                .iter_mut()
                .find(|gpu| gpu.physical_device.as_ref() == Some(device))
            else {
                return Some(temperature.snapshot);
            };

            if gpu.snapshot.temperature_celsius.is_none() {
                gpu.snapshot.temperature_celsius = Some(temperature.snapshot.celsius);
            }
            None
        })
        .collect()
}

impl Default for Backend {
    fn default() -> Self {
        Self::new()
    }
}

impl CoreCollector for Backend {
    fn collect(&mut self) -> RawSnapshot {
        let collected_at = Instant::now();
        let cpu = devlog::timed("collector.cpu", || cpu::collect(None));
        let memory = devlog::timed("collector.memory", || memory::collect(None));
        let processes = devlog::timed("collector.processes", || self.processes.collect(None));
        self.active_processes.clear();
        let active_processes = if let Collection::Available(processes) = &processes {
            self.active_processes
                .extend(processes.iter().map(|process| process.process));
            Some(&self.active_processes)
        } else {
            None
        };
        let networks = devlog::timed("collector.network", || network::collect(None));
        let disks = devlog::timed("collector.disk", || disk::collect(None));
        let process_disk_io = devlog::timed("collector.disk-attribution", || {
            self.ebpf_io
                .collect_disk(active_processes, None)
                .map(|result| result.rows)
        });
        let process_network_io = devlog::timed("collector.network-attribution", || {
            self.ebpf_io
                .collect_network(active_processes, None)
                .map(|result| result.rows)
        });
        let (gpus, temperatures) = self.collect_gpu_temperatures(None);
        let fans = devlog::timed("collector.fans", || self.fans.collect(Instant::now(), None));

        RawSnapshot {
            collected_at,
            cpu,
            memory,
            processes,
            networks,
            disks,
            process_disk_io,
            process_network_io,
            temperatures,
            fans,
            gpus,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cclover_core::model::{CollectionUnavailable, GpuId};

    fn gpu(device: &str, temperature_celsius: Option<f64>) -> gpu::Observation {
        gpu::Observation {
            physical_device: Some(PhysicalDeviceId(PathBuf::from(device))),
            snapshot: GpuSnapshot {
                id: GpuId::from_opaque_key(format!("gpu:{device}")),
                name: "GPU".to_owned(),
                utilization_percent: None,
                memory_used_bytes: None,
                memory_total_bytes: None,
                temperature_celsius,
                power_watts: None,
                core_clock_mhz: None,
                fan_percent: None,
                fan_rpm: None,
            },
        }
    }

    fn temperature(device: Option<&str>, id: &str, value: f64) -> temperature::Observation {
        temperature::Observation {
            physical_device: device.map(|value| PhysicalDeviceId(PathBuf::from(value))),
            snapshot: TemperatureSnapshot {
                id: cclover_core::model::TemperatureId::from_opaque_key(id),
                name: "GPU".to_owned(),
                celsius: value,
            },
        }
    }

    #[test]
    fn matching_hwmon_temperature_is_owned_by_gpu() {
        let (gpus, temperatures) = reconcile_gpu_temperatures(
            Collection::available(vec![gpu("/device/a", None)]),
            Collection::available(vec![temperature(Some("/device/a"), "temp-a", 63.0)]),
        );

        assert_eq!(gpus.value().unwrap()[0].temperature_celsius, Some(63.0));
        assert!(temperatures.value().unwrap().is_empty());
    }

    #[test]
    fn existing_gpu_temperature_wins_but_matching_hwmon_value_is_still_consumed() {
        let (gpus, temperatures) = reconcile_gpu_temperatures(
            Collection::available(vec![gpu("/device/a", Some(60.0))]),
            Collection::available(vec![temperature(Some("/device/a"), "temp-a", 63.0)]),
        );

        assert_eq!(gpus.value().unwrap()[0].temperature_celsius, Some(60.0));
        assert!(temperatures.value().unwrap().is_empty());
    }

    #[test]
    fn unmatched_temperature_remains_generic_and_is_normalized_after_reconciliation() {
        let (_, temperatures) = reconcile_gpu_temperatures(
            Collection::available(vec![gpu("/device/a", None)]),
            Collection::available(vec![
                temperature(Some("/device/a"), "owned", 63.0),
                temperature(Some("/device/b"), "generic", 55.0),
            ]),
        );

        let values = temperatures.value().unwrap();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].id.as_opaque_key(), "generic");
        assert_eq!(values[0].name, "GPU");
    }

    #[test]
    fn unavailable_gpu_collection_never_suppresses_temperature() {
        let (_, temperatures) = reconcile_gpu_temperatures(
            Collection::unavailable(CollectionUnavailable::Unsupported),
            Collection::available(vec![temperature(Some("/device/a"), "temp-a", 63.0)]),
        );

        assert_eq!(temperatures.value().unwrap().len(), 1);
        assert_eq!(
            temperatures.value().unwrap()[0].id.as_opaque_key(),
            "temp-a"
        );
    }
}
