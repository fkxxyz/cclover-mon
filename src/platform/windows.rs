mod cpu;
mod diagnostics;
mod disk;
mod memory;
mod native;
mod network;
mod process;

use std::time::Instant;

use crate::core::Collector as CoreCollector;
use crate::core::devlog;
use crate::core::model::{Collection, CollectionUnavailable, RawSnapshot};
use crate::platform::probe::ProbeSample;
use crate::platform::{ProbeKind, ProbeReport};

pub struct Backend;

impl Backend {
    pub fn new() -> Self {
        Self
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
        notes: Option<&mut Vec<String>>,
    ) -> ProbeSample {
        match kind {
            ProbeKind::Cpu => ProbeSample::Cpu(cpu::collect(notes)),
            ProbeKind::Memory => ProbeSample::Memory(memory::collect(notes)),
            ProbeKind::Processes => ProbeSample::Processes(process::collect(notes)),
            ProbeKind::Network => ProbeSample::Network(network::collect(notes)),
            ProbeKind::Disk => ProbeSample::Disk(disk::collect(notes)),
            ProbeKind::NetworkAttribution
            | ProbeKind::DiskAttribution
            | ProbeKind::Temperatures
            | ProbeKind::GpuMemory => ProbeSample::Unsupported(kind),
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
        RawSnapshot {
            collected_at,
            cpu: devlog::timed("collector.cpu", || cpu::collect(None)),
            memory: devlog::timed("collector.memory", || memory::collect(None)),
            processes: devlog::timed("collector.processes", || process::collect(None)),
            networks: devlog::timed("collector.network", || network::collect(None)),
            disks: devlog::timed("collector.disk", || disk::collect(None)),
            process_disk_io: Collection::unavailable(CollectionUnavailable::Unsupported),
            process_network_io: Collection::unavailable(CollectionUnavailable::Unsupported),
            temperatures: Collection::unavailable(CollectionUnavailable::Unsupported),
            gpu_memory: Collection::unavailable(CollectionUnavailable::Unsupported),
        }
    }
}
