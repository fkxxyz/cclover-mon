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
use crate::platform::{ProbeKind, ProbeReport};

pub struct Backend;

impl Backend {
    pub fn new() -> Self {
        Self
    }

    pub fn probe(&mut self, kind: ProbeKind) -> ProbeReport {
        let mut notes = Vec::new();
        match kind {
            ProbeKind::Cpu => report_cpu(cpu::collect(Some(&mut notes)), notes),
            ProbeKind::Memory => report_memory(memory::collect(Some(&mut notes)), notes),
            ProbeKind::Processes => report_processes(process::collect(Some(&mut notes)), notes),
            ProbeKind::Network => report_networks(network::collect(Some(&mut notes)), notes),
            ProbeKind::Disk => report_disks(disk::collect(Some(&mut notes)), notes),
            ProbeKind::NetworkAttribution
            | ProbeKind::DiskAttribution
            | ProbeKind::Temperatures => unsupported_report(kind),
        }
    }

    pub fn collect_for_perf(&mut self, kind: ProbeKind) {
        match kind {
            ProbeKind::Cpu => {
                std::hint::black_box(cpu::collect(None));
            }
            ProbeKind::Memory => {
                std::hint::black_box(memory::collect(None));
            }
            ProbeKind::Processes => {
                std::hint::black_box(process::collect(None));
            }
            ProbeKind::Network => {
                std::hint::black_box(network::collect(None));
            }
            ProbeKind::Disk => {
                std::hint::black_box(disk::collect(None));
            }
            ProbeKind::NetworkAttribution
            | ProbeKind::DiskAttribution
            | ProbeKind::Temperatures => {
                std::hint::black_box(Collection::<Vec<()>>::unavailable(
                    CollectionUnavailable::Unsupported,
                ));
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
        }
    }
}

fn unsupported_report(kind: ProbeKind) -> ProbeReport {
    ProbeReport {
        status: crate::core::model::CollectionStatus::Unavailable(
            CollectionUnavailable::Unsupported,
        ),
        summary: vec![format!(
            "{} collector is not implemented on Windows yet",
            kind.as_str()
        )],
        raw: Vec::new(),
        notes: Vec::new(),
    }
}

fn report_cpu(
    value: Collection<crate::core::model::CpuCounter>,
    notes: Vec<String>,
) -> ProbeReport {
    let status = value.status();
    let raw = value.value().map_or_else(Vec::new, |cpu| {
        vec![format!(
            "total_time_units={} idle_time_units={} logical_cpu_count={}",
            cpu.total_time_units, cpu.idle_time_units, cpu.logical_cpu_count
        )]
    });
    let summary = value.value().map_or_else(
        || vec!["CPU counters unavailable".to_owned()],
        |cpu| vec![format!("{} logical CPUs", cpu.logical_cpu_count)],
    );
    ProbeReport {
        status,
        summary,
        raw,
        notes,
    }
}

fn report_memory(
    value: Collection<crate::core::model::MemorySnapshot>,
    notes: Vec<String>,
) -> ProbeReport {
    let status = value.status();
    let rows = value.value().map_or_else(Vec::new, |memory| {
        vec![format!(
            "used_bytes={} total_bytes={} swap_used_bytes={} swap_total_bytes={}",
            memory.used_bytes, memory.total_bytes, memory.swap_used_bytes, memory.swap_total_bytes
        )]
    });
    ProbeReport {
        status,
        summary: rows.clone(),
        raw: rows,
        notes,
    }
}

fn report_processes(
    value: Collection<Vec<crate::core::model::ProcessCounter>>,
    notes: Vec<String>,
) -> ProbeReport {
    let status = value.status();
    let values = value.value().map(Vec::as_slice).unwrap_or_default();
    ProbeReport {
        status,
        summary: vec![format!("{} processes", values.len())],
        raw: values
            .iter()
            .map(|process| {
                format!(
                    "pid={} name={:?} cpu_time_units={} rss_bytes={}",
                    process.process.pid, process.name, process.cpu_time_units, process.rss_bytes
                )
            })
            .collect(),
        notes,
    }
}

fn report_networks(
    value: Collection<Vec<crate::core::model::NetworkCounter>>,
    notes: Vec<String>,
) -> ProbeReport {
    let status = value.status();
    let values = value.value().map(Vec::as_slice).unwrap_or_default();
    ProbeReport {
        status,
        summary: vec![format!("{} interfaces", values.len())],
        raw: values
            .iter()
            .map(|item| {
                format!(
                    "name={} rx_bytes={} tx_bytes={}",
                    item.name, item.rx_bytes, item.tx_bytes
                )
            })
            .collect(),
        notes,
    }
}

fn report_disks(
    value: Collection<Vec<crate::core::model::DiskCounter>>,
    notes: Vec<String>,
) -> ProbeReport {
    let status = value.status();
    let values = value.value().map(Vec::as_slice).unwrap_or_default();
    ProbeReport {
        status,
        summary: vec![format!("{} disks", values.len())],
        raw: values
            .iter()
            .map(|item| {
                format!(
                    "name={} read_bytes={} write_bytes={}",
                    item.name, item.read_bytes, item.write_bytes
                )
            })
            .collect(),
        notes,
    }
}
