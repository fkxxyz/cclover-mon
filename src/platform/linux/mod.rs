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

        match kind {
            ProbeKind::Cpu => {
                let value = cpu::collect(Some(&mut notes));
                let status = value.status();
                let summary = value
                    .value()
                    .map(|cpu| vec![format!("{} logical CPUs", cpu.logical_cpu_count)])
                    .unwrap_or_else(|| vec!["CPU counters unavailable".to_owned()]);
                let raw = value
                    .value()
                    .map(|cpu| {
                        vec![format!(
                            "total_time_units={} idle_time_units={} logical_cpu_count={}",
                            cpu.total_time_units, cpu.idle_time_units, cpu.logical_cpu_count
                        )]
                    })
                    .unwrap_or_default();
                ProbeReport {
                    status,
                    summary,
                    raw,
                    notes,
                }
            }
            ProbeKind::Memory => {
                let value = memory::collect(Some(&mut notes));
                let status = value.status();
                let summary = value
                    .value()
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
                    .value()
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
                    status,
                    summary,
                    raw,
                    notes,
                }
            }
            ProbeKind::Processes => {
                let outcome = self.processes.collect(Some(&mut notes));
                let status = outcome.status();
                let values = outcome.value().map(Vec::as_slice).unwrap_or_default();
                let raw = values
                    .iter()
                    .map(|process| {
                        format!(
                            "pid={} name={:?} cpu_time_units={} rss_bytes={}",
                            process.process.pid,
                            process.name,
                            process.cpu_time_units,
                            process.rss_bytes
                        )
                    })
                    .collect();
                ProbeReport {
                    status,
                    summary: vec![format!("{} processes", values.len())],
                    raw,
                    notes,
                }
            }
            ProbeKind::Network => {
                let outcome = network::collect(Some(&mut notes));
                let status = outcome.status();
                let values = outcome.value().map(Vec::as_slice).unwrap_or_default();
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
                    status,
                    summary: vec![format!(
                        "{} interfaces: {}",
                        values.len(),
                        names(values.iter().map(|item| item.name.as_str()))
                    )],
                    raw,
                    notes,
                }
            }
            ProbeKind::NetworkAttribution => {
                let first = self.ebpf_io.collect_network(None, Some(&mut notes));
                let outcome = if first.is_observable() {
                    std::thread::sleep(crate::core::SAMPLE_INTERVAL);
                    self.ebpf_io.collect_network(None, Some(&mut notes))
                } else {
                    first
                };
                let status = outcome.status();
                let Some(result) = outcome.value() else {
                    return ProbeReport {
                        status,
                        summary: vec!["network attribution unavailable".to_owned()],
                        raw: Vec::new(),
                        notes,
                    };
                };
                ProbeReport {
                    status,
                    summary: vec![format!("{} PID×interface rows", result.rows.len())],
                    raw: result
                        .rows
                        .iter()
                        .map(|row| {
                            format!(
                                "pid={} interface={} rx_bytes={} tx_bytes={}",
                                row.process.pid, row.interface, row.rx_bytes, row.tx_bytes
                            )
                        })
                        .collect(),
                    notes,
                }
            }
            ProbeKind::Disk => {
                let outcome = disk::collect(Some(&mut notes));
                let status = outcome.status();
                let values = outcome.value().map(Vec::as_slice).unwrap_or_default();
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
                    status,
                    summary: vec![format!(
                        "{} devices: {}",
                        values.len(),
                        names(values.iter().map(|item| item.name.as_str()))
                    )],
                    raw,
                    notes,
                }
            }
            ProbeKind::DiskAttribution => {
                let first = self.ebpf_io.collect_disk(None, Some(&mut notes));
                let outcome = if first.is_observable() {
                    std::thread::sleep(crate::core::SAMPLE_INTERVAL);
                    self.ebpf_io.collect_disk(None, Some(&mut notes))
                } else {
                    first
                };
                let status = outcome.status();
                let Some(result) = outcome.value() else {
                    return ProbeReport {
                        status,
                        summary: vec!["disk attribution unavailable".to_owned()],
                        raw: Vec::new(),
                        notes,
                    };
                };
                ProbeReport {
                    status,
                    summary: vec![format!("{} PID×device rows", result.rows.len())],
                    raw: result
                        .rows
                        .iter()
                        .map(|row| {
                            format!(
                                "pid={} device={} read_bytes={} write_bytes={}",
                                row.process.pid, row.device, row.read_bytes, row.write_bytes
                            )
                        })
                        .collect(),
                    notes,
                }
            }
            ProbeKind::Temperatures => {
                let outcome = self.temperatures.collect(Instant::now(), Some(&mut notes));
                let status = outcome.status();
                let values = outcome.value().map(Vec::as_slice).unwrap_or_default();
                let raw = values
                    .iter()
                    .map(|temperature| {
                        format!(
                            "id={} name={} celsius={:.3}",
                            temperature.id, temperature.name, temperature.celsius
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
                    status,
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
            ProbeKind::NetworkAttribution => {
                let _ = std::hint::black_box(self.ebpf_io.collect_network(None, None));
            }
            ProbeKind::Disk => {
                std::hint::black_box(disk::collect(None));
            }
            ProbeKind::DiskAttribution => {
                let _ = std::hint::black_box(self.ebpf_io.collect_disk(None, None));
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

fn names<'a>(values: impl Iterator<Item = &'a str>) -> String {
    let names = values.collect::<Vec<_>>();
    if names.is_empty() {
        "none".to_owned()
    } else {
        names.join(", ")
    }
}
