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

use crate::core::Collector as CoreCollector;
use crate::core::devlog;
use crate::core::model::RawSnapshot;
use crate::platform::probe::ProbeSample;
use crate::platform::{ProbeKind, ProbeReport};

pub fn early_command_exit_code() -> Option<i32> {
    provision::early_command_exit_code()
}

pub fn prepare_machine_capability() {
    let _ = provision::prepare_machine_capability();
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
            std::thread::sleep(crate::core::SAMPLE_INTERVAL);
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
        let processes = devlog::timed("collector.processes", || process::collect(None));
        let disks = devlog::timed("collector.disk", || disk::collect_batch(None));
        let process_disk_io = devlog::timed("collector.disk-attribution", || {
            self.disk_attribution.collect(&processes, &disks, None)
        });
        let process_network_io = devlog::timed("collector.network-attribution", || {
            self.network_attribution.collect(&processes, None)
        });
        let hardware = devlog::timed("collector.hardware", || self.hardware.collect(None));
        let native_temperatures = devlog::timed("collector.temperature.native", || {
            self.temperatures.collect(None)
        });
        RawSnapshot {
            collected_at,
            cpu: devlog::timed("collector.cpu", || cpu::collect(None)),
            memory: devlog::timed("collector.memory", || memory::collect(None)),
            processes,
            networks,
            disks: disks.counters,
            process_disk_io,
            process_network_io,
            temperatures: hardware::merge_temperature_sources([
                hardware.temperatures,
                native_temperatures,
            ]),
            fans: hardware.fans,
            gpus: devlog::timed("collector.gpu", || self.gpus.collect(None)),
        }
    }
}
