mod cpu;
mod diagnostics;
mod disk;
mod disk_attribution;
mod etw;
mod gpu;
mod gpu_adl;
mod gpu_d3dkmt;
mod gpu_nvml;
mod hardware;
mod memory;
mod native;
mod ndu;
mod network;
mod network_attribution;
mod network_device;
mod pawnio;
mod process;
mod provision;
mod temperature;

use std::time::Instant;

use crate::probe::ProbeSample;
use crate::{ProbeKind, ProbeReport};
use cclover_core::Collector as CoreCollector;
use cclover_core::devlog;
use cclover_core::model::{
    Collection, ProcessCounter, ProcessDiskIoCounter, ProcessNetworkIoCounter, RawSnapshot,
    TemperatureSnapshot,
};

pub fn early_command_exit_code() -> Option<i32> {
    provision::early_command_exit_code()
}

pub fn prepare_machine_capability() {
    let _ = provision::prepare_machine_capability();
}

#[cfg(feature = "windows-etw-validation")]
pub(super) fn run_etw_semantic_validation() -> Result<(), String> {
    disk_attribution::run_native_semantic_validation()
}

pub struct Backend {
    disk_attribution: disk_attribution::Collector,
    gpus: gpu::Collector,
    hardware: hardware::Collector,
    network: network::Collector,
    network_attribution: network_attribution::Collector,
    temperatures: temperature::Collector,
}

impl Backend {
    pub fn new() -> Self {
        Self {
            disk_attribution: disk_attribution::Collector::new(),
            gpus: gpu::Collector::new(),
            hardware: hardware::Collector::new(),
            network: network::Collector::new(),
            network_attribution: network_attribution::Collector::new(),
            temperatures: temperature::Collector::new(),
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
        // Keep this match exhaustive. Perf uses production projections and prepares the
        // minimum production context required by stateful attribution collectors; probe
        // projections intentionally remain independent diagnostics.
        let sample = match kind {
            ProbeKind::Cpu => ProbeSample::Cpu(cpu::collect(None)),
            ProbeKind::Memory => ProbeSample::Memory(memory::collect(None)),
            ProbeKind::Processes => ProbeSample::Processes(Self::collect_product_processes()),
            ProbeKind::Network => ProbeSample::Network(self.network.collect(None)),
            ProbeKind::NetworkAttribution => {
                let processes = Self::collect_product_processes();
                ProbeSample::NetworkAttribution(
                    self.collect_product_network_attribution(&processes),
                )
            }
            ProbeKind::Disk => ProbeSample::Disk(Self::collect_product_disks().counters),
            ProbeKind::DiskAttribution => {
                let processes = Self::collect_product_processes();
                let disks = Self::collect_product_disks();
                ProbeSample::DiskAttribution(
                    self.collect_product_disk_attribution(&processes, &disks),
                )
            }
            ProbeKind::Temperatures => {
                let hardware = self.collect_product_hardware();
                ProbeSample::Temperatures(self.collect_product_temperatures(hardware.temperatures))
            }
            ProbeKind::Fans => ProbeSample::Fans(self.collect_product_hardware().fans),
            ProbeKind::Gpu => ProbeSample::Gpu(self.gpus.collect(None)),
        };
        std::hint::black_box(sample);
    }

    fn collect_product_processes() -> Collection<Vec<ProcessCounter>> {
        process::collect(None)
    }

    fn collect_product_disks() -> disk::Batch {
        disk::collect_batch(None)
    }

    fn collect_product_hardware(&mut self) -> hardware::Batch {
        self.hardware.collect(None)
    }

    fn collect_product_temperatures(
        &self,
        hardware_temperatures: Collection<Vec<TemperatureSnapshot>>,
    ) -> Collection<Vec<TemperatureSnapshot>> {
        let native_temperatures = devlog::timed("collector.temperature.native", || {
            self.temperatures.collect(None)
        });
        hardware::merge_temperature_sources([hardware_temperatures, native_temperatures])
    }

    fn collect_product_network_attribution(
        &mut self,
        processes: &Collection<Vec<ProcessCounter>>,
    ) -> Collection<Vec<ProcessNetworkIoCounter>> {
        self.network_attribution.collect(processes, None)
    }

    fn collect_product_disk_attribution(
        &mut self,
        processes: &Collection<Vec<ProcessCounter>>,
        disks: &disk::Batch,
    ) -> Collection<Vec<ProcessDiskIoCounter>> {
        self.disk_attribution.collect(processes, disks, None)
    }

    fn collect_probe_once(
        &mut self,
        kind: ProbeKind,
        mut notes: Option<&mut Vec<String>>,
    ) -> ProbeSample {
        match kind {
            ProbeKind::Cpu => ProbeSample::Cpu(cpu::collect(notes)),
            ProbeKind::Memory => ProbeSample::Memory(memory::collect(notes)),
            ProbeKind::Processes => ProbeSample::Processes(process::collect(notes)),
            ProbeKind::Network => ProbeSample::Network(self.network.collect(notes)),
            ProbeKind::Disk => ProbeSample::Disk(disk::collect(notes)),
            ProbeKind::Temperatures => {
                let hardware = self.hardware.collect_temperatures(notes.as_deref_mut());
                let native = self.temperatures.collect_diagnostic(notes);
                ProbeSample::Temperatures(hardware::merge_temperature_sources([hardware, native]))
            }
            ProbeKind::Fans => ProbeSample::Fans(self.hardware.collect_fans(notes)),
            ProbeKind::Gpu => ProbeSample::Gpu(self.gpus.collect(notes)),
            ProbeKind::NetworkAttribution => {
                let processes = process::collect(notes.as_deref_mut());
                ProbeSample::NetworkAttribution(self.network_attribution.collect(&processes, notes))
            }
            ProbeKind::DiskAttribution => {
                let processes = process::collect(notes.as_deref_mut());
                let disks = disk::collect_batch(notes.as_deref_mut());
                ProbeSample::DiskAttribution(
                    self.disk_attribution.collect(&processes, &disks, notes),
                )
            }
        }
    }
}

impl Default for Backend {
    fn default() -> Self {
        Self::new()
    }
}

impl CoreCollector for Backend {
    fn collect(&mut self) -> RawSnapshot {
        let collected_at = Instant::now();
        let networks = devlog::timed("collector.network", || self.network.collect(None));
        let processes = devlog::timed("collector.processes", Self::collect_product_processes);
        let disks = devlog::timed("collector.disk", Self::collect_product_disks);
        let process_disk_io = devlog::timed("collector.disk-attribution", || {
            self.collect_product_disk_attribution(&processes, &disks)
        });
        let process_network_io = devlog::timed("collector.network-attribution", || {
            self.collect_product_network_attribution(&processes)
        });
        let hardware = devlog::timed("collector.hardware", || self.collect_product_hardware());
        let temperatures = self.collect_product_temperatures(hardware.temperatures);
        RawSnapshot {
            collected_at,
            cpu: devlog::timed("collector.cpu", || cpu::collect(None)),
            memory: devlog::timed("collector.memory", || memory::collect(None)),
            processes,
            networks,
            disks: disks.counters,
            process_disk_io,
            process_network_io,
            temperatures,
            fans: hardware.fans,
            gpus: devlog::timed("collector.gpu", || self.gpus.collect(None)),
        }
    }
}
