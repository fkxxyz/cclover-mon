mod cpu;
mod diagnostics;
mod disk;
mod memory;
mod network;
mod process;
mod temperature;

use std::time::Instant;

use crate::core::Collector as CoreCollector;
use crate::core::devlog;
use crate::core::model::RawSnapshot;
use crate::platform::{ProbeKind, ProbeReport};

pub struct Backend {
    processes: process::Collector,
    temperatures: temperature::Collector,
}

impl Backend {
    pub fn new() -> Self {
        Self {
            processes: process::Collector::new(),
            temperatures: temperature::Collector::new(),
        }
    }

    pub fn probe(&mut self, kind: ProbeKind) -> ProbeReport {
        let mut notes = Vec::new();

        match kind {
            ProbeKind::Cpu => {
                let value = cpu::collect(Some(&mut notes));
                let available = value.is_some();
                let summary = value
                    .as_ref()
                    .map(|cpu| vec![format!("{} logical CPUs", cpu.logical_cpu_count)])
                    .unwrap_or_else(|| vec!["CPU counters unavailable".to_owned()]);
                let raw = value
                    .map(|cpu| {
                        vec![format!(
                            "total_jiffies={} idle_jiffies={} logical_cpu_count={}",
                            cpu.total_jiffies, cpu.idle_jiffies, cpu.logical_cpu_count
                        )]
                    })
                    .unwrap_or_default();
                ProbeReport {
                    available,
                    summary,
                    raw,
                    notes,
                }
            }
            ProbeKind::Memory => {
                let value = memory::collect(Some(&mut notes));
                let available = value.is_some();
                let summary = value
                    .as_ref()
                    .map(|memory| {
                        vec![format!(
                            "used_bytes={} total_bytes={} swap_used_bytes={} swap_total_bytes={}",
                            memory.used_bytes,
                            memory.total_bytes,
                            memory.swap_used_bytes,
                            memory.swap_total_bytes
                        )]
                    })
                    .unwrap_or_else(|| vec!["memory counters unavailable".to_owned()]);
                let raw = value
                    .map(|memory| {
                        vec![format!(
                            "used_bytes={} total_bytes={} swap_used_bytes={} swap_total_bytes={}",
                            memory.used_bytes,
                            memory.total_bytes,
                            memory.swap_used_bytes,
                            memory.swap_total_bytes
                        )]
                    })
                    .unwrap_or_default();
                ProbeReport {
                    available,
                    summary,
                    raw,
                    notes,
                }
            }
            ProbeKind::Processes => {
                let values = self.processes.collect(Some(&mut notes));
                let raw = values
                    .iter()
                    .map(|process| {
                        format!(
                            "pid={} name={:?} cpu_ticks={} rss_bytes={}",
                            process.pid, process.name, process.cpu_ticks, process.rss_bytes
                        )
                    })
                    .collect();
                ProbeReport {
                    available: collection_available(&values, &notes),
                    summary: vec![format!("{} processes", values.len())],
                    raw,
                    notes,
                }
            }
            ProbeKind::Network => {
                let values = network::collect(Some(&mut notes));
                let raw = values
                    .iter()
                    .map(|network| {
                        format!(
                            "name={} rx_bytes={} tx_bytes={}",
                            network.name, network.rx_bytes, network.tx_bytes
                        )
                    })
                    .collect();
                ProbeReport {
                    available: collection_available(&values, &notes),
                    summary: vec![format!(
                        "{} interfaces: {}",
                        values.len(),
                        names(values.iter().map(|item| item.name.as_str()))
                    )],
                    raw,
                    notes,
                }
            }
            ProbeKind::Disk => {
                let values = disk::collect(Some(&mut notes));
                let raw = values
                    .iter()
                    .map(|disk| {
                        format!(
                            "name={} read_bytes={} write_bytes={}",
                            disk.name, disk.read_bytes, disk.write_bytes
                        )
                    })
                    .collect();
                ProbeReport {
                    available: collection_available(&values, &notes),
                    summary: vec![format!(
                        "{} devices: {}",
                        values.len(),
                        names(values.iter().map(|item| item.name.as_str()))
                    )],
                    raw,
                    notes,
                }
            }
            ProbeKind::Temperatures => {
                let values = self.temperatures.collect(Instant::now(), Some(&mut notes));
                let raw = values
                    .iter()
                    .map(|temperature| {
                        format!(
                            "name={} celsius={:.3}",
                            temperature.name, temperature.celsius
                        )
                    })
                    .collect();
                let summary = if values.is_empty() {
                    vec!["0 temperature sensors".to_owned()]
                } else {
                    values
                        .iter()
                        .map(|temperature| {
                            format!("{} {:.1}°C", temperature.name, temperature.celsius)
                        })
                        .collect()
                };
                ProbeReport {
                    available: collection_available(&values, &notes),
                    summary,
                    raw,
                    notes,
                }
            }
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
                std::hint::black_box(self.processes.collect(None));
            }
            ProbeKind::Network => {
                std::hint::black_box(network::collect(None));
            }
            ProbeKind::Disk => {
                std::hint::black_box(disk::collect(None));
            }
            ProbeKind::Temperatures => {
                std::hint::black_box(self.temperatures.collect(Instant::now(), None));
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
        let networks = devlog::timed("collector.network", || network::collect(None));
        let disks = devlog::timed("collector.disk", || disk::collect(None));
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
            temperatures,
        }
    }
}

fn collection_available<T>(values: &[T], notes: &[String]) -> bool {
    !values.is_empty() || !notes.iter().any(|note| note.starts_with("cannot read "))
}

fn names<'a>(values: impl Iterator<Item = &'a str>) -> String {
    let names = values.collect::<Vec<_>>();
    if names.is_empty() {
        "none".to_owned()
    } else {
        names.join(", ")
    }
}
