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
use cclover_core::model::{
    Collection, GpuSnapshot, ProcessCounter, ProcessDiskIoCounter, ProcessInstanceId,
    ProcessNetworkIoCounter, RawSnapshot, TemperatureSnapshot,
};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct PhysicalDeviceId(PathBuf);

impl PhysicalDeviceId {
    fn from_path(path: &Path) -> Option<Self> {
        std::fs::canonicalize(path).ok().map(Self)
    }
}

#[derive(Default)]
struct ActiveProcessContext {
    processes: HashSet<ProcessInstanceId>,
    complete: bool,
}

struct PreparedProcesses {
    snapshot: Collection<Vec<ProcessCounter>>,
}

impl ActiveProcessContext {
    fn refresh(&mut self, snapshot: &Collection<Vec<ProcessCounter>>) {
        self.processes.clear();
        self.complete = false;
        if let Collection::Available(processes) = snapshot {
            self.processes
                .extend(processes.iter().map(|process| process.process));
            self.complete = true;
        }
    }

    fn current(&self) -> Option<&HashSet<ProcessInstanceId>> {
        self.complete.then_some(&self.processes)
    }
}

pub struct Backend {
    processes: process::Collector,
    temperatures: temperature::Collector,
    fans: fan::Collector,
    nvidia: nvidia::Collector,
    ebpf_io: ebpf_io::Collector,
    active_processes: ActiveProcessContext,
}

impl Backend {
    pub fn new() -> Self {
        Self {
            processes: process::Collector::new(),
            temperatures: temperature::Collector::new(),
            fans: fan::Collector::new(),
            nvidia: nvidia::Collector::new(),
            ebpf_io: ebpf_io::Collector::new(),
            active_processes: ActiveProcessContext::default(),
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
        // Keep this match exhaustive. Perf uses production composition; probe orchestration is
        // intentionally separate because its diagnostic projections may differ.
        let sample = match kind {
            ProbeKind::Cpu => ProbeSample::Cpu(cpu::collect(None)),
            ProbeKind::Memory => ProbeSample::Memory(memory::collect(None)),
            ProbeKind::Processes => ProbeSample::Processes(self.collect_processes(None)),
            ProbeKind::Network => ProbeSample::Network(network::collect(None)),
            ProbeKind::NetworkAttribution => {
                let processes = self.prepare_product_processes();
                ProbeSample::NetworkAttribution(
                    self.collect_product_network_attribution(&processes),
                )
            }
            ProbeKind::Disk => ProbeSample::Disk(disk::collect(None)),
            ProbeKind::DiskAttribution => {
                let processes = self.prepare_product_processes();
                ProbeSample::DiskAttribution(self.collect_product_disk_attribution(&processes))
            }
            ProbeKind::Temperatures => {
                let (_, temperatures) = self.collect_gpu_temperatures(None);
                ProbeSample::Temperatures(temperatures)
            }
            ProbeKind::Fans => ProbeSample::Fans(self.fans.collect(Instant::now(), None)),
            ProbeKind::Gpu => {
                let (gpus, _) = self.collect_gpu_temperatures(None);
                ProbeSample::Gpu(gpus)
            }
        };
        std::hint::black_box(sample);
    }

    fn prepare_product_processes(&mut self) -> PreparedProcesses {
        PreparedProcesses {
            snapshot: self.collect_processes(None),
        }
    }

    fn collect_product_disk_attribution(
        &mut self,
        _processes: &PreparedProcesses,
    ) -> Collection<Vec<ProcessDiskIoCounter>> {
        self.ebpf_io
            .collect_disk(self.active_processes.current(), None)
            .map(|result| result.rows)
    }

    fn collect_product_network_attribution(
        &mut self,
        _processes: &PreparedProcesses,
    ) -> Collection<Vec<ProcessNetworkIoCounter>> {
        self.ebpf_io
            .collect_network(self.active_processes.current(), None)
            .map(|result| result.rows)
    }

    fn collect_processes(
        &mut self,
        notes: Option<&mut Vec<String>>,
    ) -> Collection<Vec<ProcessCounter>> {
        let snapshot = self.processes.collect(notes);
        self.active_processes.refresh(&snapshot);
        snapshot
    }

    fn collect_probe_once(
        &mut self,
        kind: ProbeKind,
        notes: Option<&mut Vec<String>>,
    ) -> ProbeSample {
        match kind {
            ProbeKind::Cpu => ProbeSample::Cpu(cpu::collect(notes)),
            ProbeKind::Memory => ProbeSample::Memory(memory::collect(notes)),
            ProbeKind::Processes => ProbeSample::Processes(self.collect_processes(notes)),
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
        let processes = devlog::timed("collector.processes", || self.prepare_product_processes());
        let networks = devlog::timed("collector.network", || network::collect(None));
        let disks = devlog::timed("collector.disk", || disk::collect(None));
        let process_disk_io = devlog::timed("collector.disk-attribution", || {
            self.collect_product_disk_attribution(&processes)
        });
        let process_network_io = devlog::timed("collector.network-attribution", || {
            self.collect_product_network_attribution(&processes)
        });
        let (gpus, temperatures) = self.collect_gpu_temperatures(None);
        let fans = devlog::timed("collector.fans", || self.fans.collect(Instant::now(), None));

        RawSnapshot {
            collected_at,
            cpu,
            memory,
            processes: processes.snapshot,
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

    fn process_counter(pid: u32, birth_marker: u64) -> ProcessCounter {
        ProcessCounter {
            process: ProcessInstanceId { pid, birth_marker },
            name: std::sync::Arc::from("test"),
            cpu_time_units: 0,
            rss_bytes: 0,
        }
    }

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
    fn active_process_context_tracks_complete_process_identity() {
        let mut context = ActiveProcessContext::default();
        let first = ProcessInstanceId {
            pid: 42,
            birth_marker: 100,
        };
        let reused = ProcessInstanceId {
            pid: 42,
            birth_marker: 200,
        };

        context.refresh(&Collection::available(vec![process_counter(42, 100)]));
        assert_eq!(context.current(), Some(&HashSet::from([first])));

        context.refresh(&Collection::available(vec![process_counter(42, 200)]));
        assert_eq!(context.current(), Some(&HashSet::from([reused])));
    }

    #[test]
    fn incomplete_process_snapshot_disables_stale_retirement_context() {
        let mut context = ActiveProcessContext::default();
        context.refresh(&Collection::available(vec![process_counter(42, 100)]));
        assert!(context.current().is_some());

        context.refresh(&Collection::degraded(vec![process_counter(42, 100)]));
        assert!(context.current().is_none());

        context.refresh(&Collection::unavailable(CollectionUnavailable::Unavailable));
        assert!(context.current().is_none());
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
