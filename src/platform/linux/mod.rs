mod cpu;
mod diagnostics;
mod disk;
mod ebpf_io;
mod memory;
mod native;
mod network;
mod process;
mod temperature;

use std::collections::HashSet;
use std::time::Instant;

use crate::core::Collector as CoreCollector;
use crate::core::devlog;
use crate::core::model::{Collection, RawSnapshot};
use crate::platform::probe::ProbeSample;
use crate::platform::{ProbeKind, ProbeReport};

pub struct Backend {
    processes: process::Collector,
    temperatures: temperature::Collector,
    ebpf_io: ebpf_io::Collector,
}

impl Backend {
    pub fn new() -> Self {
        Self {
            processes: process::Collector::new(),
            temperatures: temperature::Collector::new(),
            ebpf_io: ebpf_io::Collector::new(),
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
                ProbeSample::Temperatures(self.temperatures.collect(Instant::now(), notes))
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
        let cpu = devlog::timed("collector.cpu", || cpu::collect(None));
        let memory = devlog::timed("collector.memory", || memory::collect(None));
        let processes = devlog::timed("collector.processes", || self.processes.collect(None));
        let active_processes = match &processes {
            Collection::Available(processes) => Some(
                processes
                    .iter()
                    .map(|process| process.process)
                    .collect::<HashSet<_>>(),
            ),
            Collection::Degraded(_) | Collection::Unavailable(_) => None,
        };
        let networks = devlog::timed("collector.network", || network::collect(None));
        let disks = devlog::timed("collector.disk", || disk::collect(None));
        let process_disk_io = devlog::timed("collector.disk-attribution", || {
            self.ebpf_io
                .collect_disk(active_processes.as_ref(), None)
                .map(|result| result.rows)
        });
        let process_network_io = devlog::timed("collector.network-attribution", || {
            self.ebpf_io
                .collect_network(active_processes.as_ref(), None)
                .map(|result| result.rows)
        });
        let temperatures = devlog::timed("collector.temperatures", || {
            self.temperatures.collect(Instant::now(), None)
        });

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
        }
    }
}
