use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::devlog;
use super::history;
use super::model::*;

const TOP_N: usize = 8;
const PROCESS_IO_TOP_N: usize = 3;
const HISTORY_CAPACITY: usize = 60;
pub const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy)]
pub struct SampleCycle {
    started: Instant,
}

impl SampleCycle {
    pub fn begin() -> Self {
        Self {
            started: Instant::now(),
        }
    }

    pub fn remaining(self) -> Duration {
        remaining_sample_wait(self.started.elapsed())
    }
}

fn remaining_sample_wait(elapsed: Duration) -> Duration {
    SAMPLE_INTERVAL.saturating_sub(elapsed)
}

pub trait Collector: Send + 'static {
    fn collect(&mut self) -> RawSnapshot;
}

pub struct Sampler<C> {
    collector: C,
    previous: Option<RawSnapshot>,
    state: MonitorState,
}

impl<C: Collector> Sampler<C> {
    pub fn new(collector: C) -> Self {
        Self {
            collector,
            previous: None,
            state: MonitorState {
                history_capacity: HISTORY_CAPACITY,
                ..MonitorState::default()
            },
        }
    }

    pub fn sample(&mut self) -> MonitorState {
        let started = devlog::enabled().then(Instant::now);
        let raw = self.collector.collect();
        let snapshot = derive(self.previous.as_ref(), &raw);
        history::push(
            &mut self.state.history,
            &snapshot,
            self.state.history_capacity,
        );
        self.state.snapshot = snapshot;
        self.previous = Some(raw);
        let state = self.state.clone();

        if let Some(started) = started {
            let elapsed = started.elapsed();
            devlog::log(format_args!(
                "sampling duration={:.3}ms target={:.3}ms",
                elapsed.as_secs_f64() * 1_000.0,
                SAMPLE_INTERVAL.as_secs_f64() * 1_000.0
            ));
            if elapsed > SAMPLE_INTERVAL {
                devlog::log(format_args!(
                    "sampling overrun actual={:.3}ms target={:.3}ms",
                    elapsed.as_secs_f64() * 1_000.0,
                    SAMPLE_INTERVAL.as_secs_f64() * 1_000.0
                ));
            }
        }

        state
    }
}

fn derive(previous: Option<&RawSnapshot>, current: &RawSnapshot) -> SystemSnapshot {
    let current_processes = current
        .processes
        .value()
        .map(Vec::as_slice)
        .unwrap_or_default();
    let (cpu_percent, process_cpu) = derive_cpu(previous, current, current_processes);
    let dt = sample_interval_seconds(previous, current);
    let processes = derive_process_domain(previous, current, current_processes, &process_cpu, dt);
    let top_cpu = collection_from_status(processes.cpu_status, top_cpu(&processes));
    let top_memory = collection_from_status(processes.memory_status, top_memory(&processes));
    let process_disk_io = collection_from_status(
        processes.disk_io_status,
        project_process_disk_io(&processes),
    );
    let process_network_io = collection_from_status(
        processes.network_io_status,
        project_process_network_io(&processes),
    );

    SystemSnapshot {
        cpu_percent,
        memory: current.memory.clone(),
        processes,
        top_cpu,
        top_memory,
        networks: derive_networks(previous, current, dt),
        disks: derive_disks(previous, current, dt),
        process_disk_io,
        process_network_io,
        temperatures: current.temperatures.clone(),
        fans: current.fans.clone(),
        gpus: current.gpus.clone(),
    }
}

fn map_status<S, T>(source: &Collection<S>, value: T) -> Collection<T> {
    match source {
        Collection::Available(_) => Collection::Available(value),
        Collection::Degraded(_) => Collection::Degraded(value),
        Collection::Unavailable(reason) => Collection::Unavailable(*reason),
    }
}

fn combine_status<A, B, T>(
    first: &Collection<A>,
    second: &Collection<B>,
    value: T,
) -> Collection<T> {
    match (first, second) {
        (Collection::Unavailable(reason), _) | (_, Collection::Unavailable(reason)) => {
            Collection::Unavailable(*reason)
        }
        (Collection::Degraded(_), _) | (_, Collection::Degraded(_)) => Collection::Degraded(value),
        (Collection::Available(_), Collection::Available(_)) => Collection::Available(value),
    }
}

fn collection_from_status<T>(status: CollectionStatus, value: T) -> Collection<T> {
    match status {
        CollectionStatus::Available => Collection::Available(value),
        CollectionStatus::Degraded => Collection::Degraded(value),
        CollectionStatus::Unavailable(reason) => Collection::Unavailable(reason),
    }
}

fn derive_cpu(
    previous: Option<&RawSnapshot>,
    current: &RawSnapshot,
    current_processes: &[ProcessCounter],
) -> (Collection<f64>, Collection<Vec<f64>>) {
    let process_status = combine_status(&current.cpu, &current.processes, ()).status();
    let Some(new) = current.cpu.value() else {
        let reason = match current.cpu.status() {
            CollectionStatus::Unavailable(reason) => reason,
            _ => unreachable!("observable collection must expose a value"),
        };
        return (
            Collection::Unavailable(reason),
            collection_from_status(process_status, Vec::new()),
        );
    };

    let Some((previous, old)) =
        previous.and_then(|snapshot| snapshot.cpu.value().map(|counter| (snapshot, counter)))
    else {
        let values = vec![0.0; current_processes.len()];
        return (
            map_status(&current.cpu, 0.0),
            collection_from_status(process_status, values),
        );
    };

    let total = new.total_time_units.saturating_sub(old.total_time_units);
    if total == 0 {
        let values = vec![0.0; current_processes.len()];
        return (
            map_status(&current.cpu, 0.0),
            collection_from_status(process_status, values),
        );
    }

    let idle = new.idle_time_units.saturating_sub(old.idle_time_units);
    let percent = (100.0 * (total.saturating_sub(idle)) as f64 / total as f64).clamp(0.0, 100.0);
    let old_processes: HashMap<ProcessInstanceId, &ProcessCounter> = previous
        .processes
        .value()
        .into_iter()
        .flatten()
        .map(|process| (process.process, process))
        .collect();
    let scale = new.logical_cpu_count.max(1) as f64 * 100.0 / total as f64;
    let values = current_processes
        .iter()
        .map(|process| {
            let delta = old_processes.get(&process.process).map_or(0, |old| {
                process.cpu_time_units.saturating_sub(old.cpu_time_units)
            });
            delta as f64 * scale
        })
        .collect();

    (
        map_status(&current.cpu, percent),
        collection_from_status(process_status, values),
    )
}

fn sample_interval_seconds(previous: Option<&RawSnapshot>, current: &RawSnapshot) -> f64 {
    previous
        .map(|snapshot| {
            current
                .collected_at
                .saturating_duration_since(snapshot.collected_at)
                .as_secs_f64()
                .max(0.001)
        })
        .unwrap_or(1.0)
}

fn derive_networks(
    previous: Option<&RawSnapshot>,
    current: &RawSnapshot,
    dt: f64,
) -> Collection<Vec<NetworkSnapshot>> {
    let old: HashMap<&NetworkId, &NetworkCounter> = previous
        .and_then(|snapshot| snapshot.networks.value())
        .into_iter()
        .flatten()
        .map(|item| (&item.id, item))
        .collect();
    let values = current
        .networks
        .value()
        .into_iter()
        .flatten()
        .map(|item| {
            let (down, up) = old.get(&item.id).map_or((0.0, 0.0), |old| {
                (
                    item.rx_bytes.saturating_sub(old.rx_bytes) as f64 / dt,
                    item.tx_bytes.saturating_sub(old.tx_bytes) as f64 / dt,
                )
            });
            NetworkSnapshot {
                id: item.id.clone(),
                name: item.name.clone(),
                down_bytes_per_sec: down,
                up_bytes_per_sec: up,
            }
        })
        .collect();
    map_status(&current.networks, values)
}

fn derive_disks(
    previous: Option<&RawSnapshot>,
    current: &RawSnapshot,
    dt: f64,
) -> Collection<Vec<DiskSnapshot>> {
    let old: HashMap<&DiskId, &DiskCounter> = previous
        .and_then(|snapshot| snapshot.disks.value())
        .into_iter()
        .flatten()
        .map(|item| (&item.id, item))
        .collect();
    let values = current
        .disks
        .value()
        .into_iter()
        .flatten()
        .map(|item| {
            let rate = old.get(&item.id).map_or(0.0, |old| {
                let old_total = old.read_bytes.saturating_add(old.write_bytes);
                let new_total = item.read_bytes.saturating_add(item.write_bytes);
                new_total.saturating_sub(old_total) as f64 / dt
            });
            DiskSnapshot {
                id: item.id.clone(),
                name: item.name.clone(),
                bytes_per_sec: rate,
            }
        })
        .collect();
    map_status(&current.disks, values)
}

fn derive_process_domain(
    previous: Option<&RawSnapshot>,
    current: &RawSnapshot,
    current_processes: &[ProcessCounter],
    process_cpu: &Collection<Vec<f64>>,
    dt: f64,
) -> ProcessDomainSnapshot {
    let mut by_id = BTreeMap::new();
    let cpu_values = process_cpu.value();

    for (index, process) in current_processes.iter().enumerate() {
        by_id.insert(
            process.process,
            ProcessSnapshot {
                id: process.process,
                name: Some(process.name.clone()),
                cpu_percent: cpu_values.and_then(|values| values.get(index).copied()),
                memory_bytes: Some(process.rss_bytes),
                disk_io: Vec::new(),
                network_io: Vec::new(),
            },
        );
    }

    let disk_rows = derive_process_disk_io_rates(
        previous.and_then(|snapshot| snapshot.process_disk_io.value().map(Vec::as_slice)),
        current
            .process_disk_io
            .value()
            .map(Vec::as_slice)
            .unwrap_or_default(),
        dt,
    );
    for (process, io) in disk_rows {
        process_entry(&mut by_id, process).disk_io.push(io);
    }

    let network_rows = derive_process_network_io_rates(
        previous.and_then(|snapshot| snapshot.process_network_io.value().map(Vec::as_slice)),
        current
            .process_network_io
            .value()
            .map(Vec::as_slice)
            .unwrap_or_default(),
        dt,
    );
    for (process, io) in network_rows {
        process_entry(&mut by_id, process).network_io.push(io);
    }

    for process in by_id.values_mut() {
        process
            .disk_io
            .sort_unstable_by(|a, b| a.disk_id.cmp(&b.disk_id));
        process
            .network_io
            .sort_unstable_by(|a, b| a.network_id.cmp(&b.network_id));
    }

    ProcessDomainSnapshot {
        by_id: Arc::new(by_id),
        metadata_status: current.processes.status(),
        cpu_status: process_cpu.status(),
        memory_status: current.processes.status(),
        disk_io_status: current.process_disk_io.status(),
        network_io_status: current.process_network_io.status(),
    }
}

fn process_entry(
    by_id: &mut BTreeMap<ProcessInstanceId, ProcessSnapshot>,
    id: ProcessInstanceId,
) -> &mut ProcessSnapshot {
    by_id.entry(id).or_insert_with(|| ProcessSnapshot {
        id,
        name: None,
        cpu_percent: None,
        memory_bytes: None,
        disk_io: Vec::new(),
        network_io: Vec::new(),
    })
}

fn derive_process_disk_io_rates(
    previous: Option<&[ProcessDiskIoCounter]>,
    current: &[ProcessDiskIoCounter],
    dt: f64,
) -> Vec<(ProcessInstanceId, ProcessDiskIoSnapshot)> {
    let old: HashMap<(ProcessInstanceId, &DiskId), &ProcessDiskIoCounter> = previous
        .into_iter()
        .flatten()
        .map(|item| ((item.process, &item.disk_id), item))
        .collect();
    current
        .iter()
        .map(|item| {
            let (read, write) = old
                .get(&(item.process, &item.disk_id))
                .map_or((0.0, 0.0), |old| {
                    (
                        item.read_bytes.saturating_sub(old.read_bytes) as f64 / dt,
                        item.write_bytes.saturating_sub(old.write_bytes) as f64 / dt,
                    )
                });
            (
                item.process,
                ProcessDiskIoSnapshot {
                    disk_id: item.disk_id.clone(),
                    device: item.device.clone(),
                    read_bytes_per_sec: read,
                    write_bytes_per_sec: write,
                },
            )
        })
        .collect()
}

fn derive_process_network_io_rates(
    previous: Option<&[ProcessNetworkIoCounter]>,
    current: &[ProcessNetworkIoCounter],
    dt: f64,
) -> Vec<(ProcessInstanceId, ProcessNetworkIoSnapshot)> {
    let old: HashMap<(ProcessInstanceId, &NetworkId), &ProcessNetworkIoCounter> = previous
        .into_iter()
        .flatten()
        .map(|item| ((item.process, &item.network_id), item))
        .collect();
    current
        .iter()
        .map(|item| {
            let (rx, tx) = old
                .get(&(item.process, &item.network_id))
                .map_or((0.0, 0.0), |old| {
                    (
                        item.rx_bytes.saturating_sub(old.rx_bytes) as f64 / dt,
                        item.tx_bytes.saturating_sub(old.tx_bytes) as f64 / dt,
                    )
                });
            (
                item.process,
                ProcessNetworkIoSnapshot {
                    network_id: item.network_id.clone(),
                    interface: item.interface.clone(),
                    rx_bytes_per_sec: rx,
                    tx_bytes_per_sec: tx,
                },
            )
        })
        .collect()
}

fn top_cpu(processes: &ProcessDomainSnapshot) -> Vec<ProcessCpuUsage> {
    let mut values: Vec<_> = processes
        .by_id
        .values()
        .filter_map(|process| Some((process, process.cpu_percent?)))
        .collect();
    keep_top_n_by(&mut values, TOP_N, |a, b| {
        b.1.total_cmp(&a.1).then_with(|| a.0.id.cmp(&b.0.id))
    });
    values
        .into_iter()
        .filter_map(|(process, percent)| {
            Some(ProcessCpuUsage {
                name: process.name.as_deref()?.to_owned(),
                percent,
            })
        })
        .collect()
}

fn top_memory(processes: &ProcessDomainSnapshot) -> Vec<ProcessMemoryUsage> {
    let mut values: Vec<_> = processes
        .by_id
        .values()
        .filter_map(|process| Some((process, process.memory_bytes?)))
        .collect();
    keep_top_n_by(&mut values, TOP_N, |a, b| {
        b.1.cmp(&a.1).then_with(|| a.0.id.cmp(&b.0.id))
    });
    values
        .into_iter()
        .filter_map(|(process, bytes)| {
            Some(ProcessMemoryUsage {
                name: process.name.as_deref()?.to_owned(),
                bytes,
            })
        })
        .collect()
}

fn project_process_disk_io(processes: &ProcessDomainSnapshot) -> Vec<ProcessDiskIo> {
    let mut by_disk: BTreeMap<DiskId, Vec<ProcessDiskIo>> = BTreeMap::new();
    for process in processes.by_id.values() {
        for io in &process.disk_io {
            by_disk
                .entry(io.disk_id.clone())
                .or_default()
                .push(ProcessDiskIo {
                    process: process.id,
                    name: process.name.clone(),
                    disk_id: io.disk_id.clone(),
                    device: io.device.clone(),
                    read_bytes_per_sec: io.read_bytes_per_sec,
                    write_bytes_per_sec: io.write_bytes_per_sec,
                });
        }
    }
    for rows in by_disk.values_mut() {
        keep_top_n_by(rows, PROCESS_IO_TOP_N, |a, b| {
            let a_total = a.read_bytes_per_sec + a.write_bytes_per_sec;
            let b_total = b.read_bytes_per_sec + b.write_bytes_per_sec;
            b_total
                .total_cmp(&a_total)
                .then_with(|| a.process.cmp(&b.process))
        });
    }
    by_disk.into_values().flatten().collect()
}

fn project_process_network_io(processes: &ProcessDomainSnapshot) -> Vec<ProcessNetworkIo> {
    let mut by_network: BTreeMap<NetworkId, Vec<ProcessNetworkIo>> = BTreeMap::new();
    for process in processes.by_id.values() {
        for io in &process.network_io {
            by_network
                .entry(io.network_id.clone())
                .or_default()
                .push(ProcessNetworkIo {
                    process: process.id,
                    name: process.name.clone(),
                    network_id: io.network_id.clone(),
                    interface: io.interface.clone(),
                    rx_bytes_per_sec: io.rx_bytes_per_sec,
                    tx_bytes_per_sec: io.tx_bytes_per_sec,
                });
        }
    }
    for rows in by_network.values_mut() {
        keep_top_n_by(rows, PROCESS_IO_TOP_N, |a, b| {
            let a_total = a.rx_bytes_per_sec + a.tx_bytes_per_sec;
            let b_total = b.rx_bytes_per_sec + b.tx_bytes_per_sec;
            b_total
                .total_cmp(&a_total)
                .then_with(|| a.process.cmp(&b.process))
        });
    }
    by_network.into_values().flatten().collect()
}

fn keep_top_n_by<T>(
    values: &mut Vec<T>,
    limit: usize,
    mut compare: impl FnMut(&T, &T) -> std::cmp::Ordering,
) {
    if values.len() > limit {
        values.select_nth_unstable_by(limit, &mut compare);
        values.truncate(limit);
    }
    values.sort_unstable_by(compare);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn process_id(pid: u32, birth_marker: u64) -> ProcessInstanceId {
        ProcessInstanceId { pid, birth_marker }
    }

    fn network_id(key: &str) -> NetworkId {
        NetworkId::from_opaque_key(key)
    }

    fn disk_id(key: &str) -> DiskId {
        DiskId::from_opaque_key(key)
    }

    #[test]
    fn sample_wait_fills_only_the_remaining_interval() {
        assert_eq!(
            remaining_sample_wait(Duration::from_millis(250)),
            Duration::from_millis(750)
        );
    }

    #[test]
    fn sample_wait_is_zero_at_or_after_the_interval() {
        assert_eq!(remaining_sample_wait(SAMPLE_INTERVAL), Duration::ZERO);
        assert_eq!(
            remaining_sample_wait(SAMPLE_INTERVAL + Duration::from_millis(1)),
            Duration::ZERO
        );
    }

    #[test]
    fn first_sample_publishes_observed_entities_with_zero_baseline_rates() {
        let current = RawSnapshot {
            cpu: Collection::available(CpuCounter {
                total_time_units: 1_000,
                idle_time_units: 600,
                logical_cpu_count: 4,
            }),
            memory: Collection::available(MemorySnapshot {
                used_bytes: 10,
                total_bytes: 20,
                swap_used_bytes: 1,
                swap_total_bytes: 2,
            }),
            processes: Collection::available(vec![ProcessCounter {
                process: process_id(10, 1),
                name: "worker".into(),
                cpu_time_units: 100,
                rss_bytes: 4096,
            }]),
            networks: Collection::available(vec![NetworkCounter {
                id: network_id("network-a"),
                name: "eth0".into(),
                rx_bytes: 1_000,
                tx_bytes: 2_000,
            }]),
            disks: Collection::available(vec![DiskCounter {
                id: disk_id("disk-a"),
                name: "nvme0n1".into(),
                read_bytes: 3_000,
                write_bytes: 4_000,
            }]),
            process_disk_io: Collection::available(vec![ProcessDiskIoCounter {
                process: process_id(10, 1),
                disk_id: disk_id("disk-a"),
                device: "nvme0n1".into(),
                read_bytes: 300,
                write_bytes: 400,
            }]),
            process_network_io: Collection::available(vec![ProcessNetworkIoCounter {
                process: process_id(10, 1),
                network_id: network_id("network-a"),
                interface: "eth0".into(),
                rx_bytes: 100,
                tx_bytes: 200,
            }]),
            temperatures: Collection::available(vec![TemperatureSnapshot {
                id: "cpu-temp".into(),
                name: "CPU".into(),
                celsius: 42.0,
            }]),
            ..RawSnapshot::default()
        };

        let out = derive(None, &current);

        assert_eq!(out.cpu_percent.value().copied(), Some(0.0));
        assert_eq!(out.top_cpu.value().unwrap().len(), 1);
        assert_eq!(out.top_cpu.value().unwrap()[0].name, "worker");
        assert_eq!(out.top_cpu.value().unwrap()[0].percent, 0.0);
        assert_eq!(out.top_memory.value().unwrap()[0].name, "worker");
        assert_eq!(out.memory.value().unwrap().used_bytes, 10);
        assert_eq!(out.temperatures.value().unwrap()[0].name, "CPU");
        assert_eq!(out.networks.value().unwrap()[0].name, "eth0");
        assert_eq!(
            (
                out.networks.value().unwrap()[0].down_bytes_per_sec,
                out.networks.value().unwrap()[0].up_bytes_per_sec,
            ),
            (0.0, 0.0)
        );
        assert_eq!(out.disks.value().unwrap()[0].name, "nvme0n1");
        assert_eq!(out.disks.value().unwrap()[0].bytes_per_sec, 0.0);

        let disk = out.process_disk_io.value().unwrap();
        assert_eq!(disk[0].device, "nvme0n1");
        assert_eq!(
            (disk[0].read_bytes_per_sec, disk[0].write_bytes_per_sec),
            (0.0, 0.0)
        );
        let network = out.process_network_io.value().unwrap();
        assert_eq!(network[0].interface, "eth0");
        assert_eq!(
            (network[0].rx_bytes_per_sec, network[0].tx_bytes_per_sec),
            (0.0, 0.0)
        );
    }

    #[test]
    fn derives_rates_and_cpu() {
        let t = Instant::now();
        let old = RawSnapshot {
            collected_at: t,
            cpu: Collection::available(CpuCounter {
                total_time_units: 1000,
                idle_time_units: 600,
                logical_cpu_count: 4,
            }),
            processes: Collection::available(vec![ProcessCounter {
                process: process_id(1, 1),
                name: "a".into(),
                cpu_time_units: 100,
                rss_bytes: 10,
            }]),
            networks: Collection::available(vec![NetworkCounter {
                id: network_id("network-a"),
                name: "eth0".into(),
                rx_bytes: 100,
                tx_bytes: 200,
            }]),
            disks: Collection::available(vec![DiskCounter {
                id: disk_id("disk-a"),
                name: "sda".into(),
                read_bytes: 100,
                write_bytes: 100,
            }]),
            ..RawSnapshot::default()
        };
        let new = RawSnapshot {
            collected_at: t + Duration::from_secs(1),
            cpu: Collection::available(CpuCounter {
                total_time_units: 1100,
                idle_time_units: 650,
                logical_cpu_count: 4,
            }),
            processes: Collection::available(vec![ProcessCounter {
                process: process_id(1, 1),
                name: "a".into(),
                cpu_time_units: 110,
                rss_bytes: 20,
            }]),
            networks: Collection::available(vec![NetworkCounter {
                id: network_id("network-a"),
                name: "eth0".into(),
                rx_bytes: 300,
                tx_bytes: 500,
            }]),
            disks: Collection::available(vec![DiskCounter {
                id: disk_id("disk-a"),
                name: "sda".into(),
                read_bytes: 300,
                write_bytes: 500,
            }]),
            ..RawSnapshot::default()
        };
        let out = derive(Some(&old), &new);
        assert_eq!(out.cpu_percent.value().copied(), Some(50.0));
        assert_eq!(out.networks.value().unwrap()[0].down_bytes_per_sec, 200.0);
        assert_eq!(out.networks.value().unwrap()[0].up_bytes_per_sec, 300.0);
        assert_eq!(out.disks.value().unwrap()[0].bytes_per_sec, 600.0);
        assert!((out.top_cpu.value().unwrap()[0].percent - 40.0).abs() < 0.001);
    }

    #[test]
    fn network_and_disk_rates_follow_identity_not_name() {
        let t = Instant::now();
        let old = RawSnapshot {
            collected_at: t,
            networks: Collection::available(vec![NetworkCounter {
                id: network_id("network-a"),
                name: "eth0".into(),
                rx_bytes: 100,
                tx_bytes: 200,
            }]),
            disks: Collection::available(vec![DiskCounter {
                id: disk_id("disk-a"),
                name: "sda".into(),
                read_bytes: 100,
                write_bytes: 100,
            }]),
            ..RawSnapshot::default()
        };
        let renamed = RawSnapshot {
            collected_at: t + Duration::from_secs(1),
            networks: Collection::available(vec![NetworkCounter {
                id: network_id("network-a"),
                name: "lan0".into(),
                rx_bytes: 300,
                tx_bytes: 500,
            }]),
            disks: Collection::available(vec![DiskCounter {
                id: disk_id("disk-a"),
                name: "system-disk".into(),
                read_bytes: 300,
                write_bytes: 500,
            }]),
            ..RawSnapshot::default()
        };

        let out = derive(Some(&old), &renamed);

        assert_eq!(out.networks.value().unwrap()[0].name, "lan0");
        assert_eq!(out.networks.value().unwrap()[0].down_bytes_per_sec, 200.0);
        assert_eq!(out.networks.value().unwrap()[0].up_bytes_per_sec, 300.0);
        assert_eq!(out.disks.value().unwrap()[0].name, "system-disk");
        assert_eq!(out.disks.value().unwrap()[0].bytes_per_sec, 600.0);
    }

    #[test]
    fn reused_names_with_new_identity_do_not_inherit_rates() {
        let t = Instant::now();
        let old = RawSnapshot {
            collected_at: t,
            networks: Collection::available(vec![NetworkCounter {
                id: network_id("network-old"),
                name: "eth0".into(),
                rx_bytes: 10_000,
                tx_bytes: 20_000,
            }]),
            disks: Collection::available(vec![DiskCounter {
                id: disk_id("disk-old"),
                name: "sda".into(),
                read_bytes: 10_000,
                write_bytes: 20_000,
            }]),
            ..RawSnapshot::default()
        };
        let replacement = RawSnapshot {
            collected_at: t + Duration::from_secs(1),
            networks: Collection::available(vec![NetworkCounter {
                id: network_id("network-new"),
                name: "eth0".into(),
                rx_bytes: 100,
                tx_bytes: 200,
            }]),
            disks: Collection::available(vec![DiskCounter {
                id: disk_id("disk-new"),
                name: "sda".into(),
                read_bytes: 100,
                write_bytes: 200,
            }]),
            ..RawSnapshot::default()
        };

        let out = derive(Some(&old), &replacement);

        assert_eq!(out.networks.value().unwrap()[0].down_bytes_per_sec, 0.0);
        assert_eq!(out.networks.value().unwrap()[0].up_bytes_per_sec, 0.0);
        assert_eq!(out.disks.value().unwrap()[0].bytes_per_sec, 0.0);
    }

    #[test]
    fn recovered_counter_observation_starts_a_new_rate_baseline() {
        let t = Instant::now();
        let unavailable = RawSnapshot {
            collected_at: t,
            networks: Collection::unavailable(CollectionUnavailable::Unavailable),
            disks: Collection::unavailable(CollectionUnavailable::Unavailable),
            ..RawSnapshot::default()
        };
        let recovered = RawSnapshot {
            collected_at: t + Duration::from_secs(1),
            networks: Collection::available(vec![NetworkCounter {
                id: network_id("network-a"),
                name: "eth0".into(),
                rx_bytes: 50_000,
                tx_bytes: 80_000,
            }]),
            disks: Collection::available(vec![DiskCounter {
                id: disk_id("disk-a"),
                name: "sda".into(),
                read_bytes: 90_000,
                write_bytes: 120_000,
            }]),
            ..RawSnapshot::default()
        };

        let out = derive(Some(&unavailable), &recovered);

        assert_eq!(out.networks.value().unwrap()[0].down_bytes_per_sec, 0.0);
        assert_eq!(out.networks.value().unwrap()[0].up_bytes_per_sec, 0.0);
        assert_eq!(out.disks.value().unwrap()[0].bytes_per_sec, 0.0);
    }

    #[test]
    fn top_memory_keeps_only_highest_processes_in_order() {
        let processes: Vec<_> = (0..12)
            .map(|index| ProcessCounter {
                process: process_id(index, 1),
                name: format!("p{index}").into(),
                cpu_time_units: 0,
                rss_bytes: u64::from(index),
            })
            .collect();
        let current = RawSnapshot {
            processes: Collection::available(processes),
            ..RawSnapshot::default()
        };

        let out = derive(None, &current);
        let top = out.top_memory.value().unwrap();
        assert_eq!(top.len(), TOP_N);
        assert_eq!(top[0].name, "p11");
        assert_eq!(top[TOP_N - 1].name, "p4");
        assert_eq!(out.processes.by_id.len(), 12);
    }

    #[test]
    fn top_cpu_keeps_only_highest_processes_in_order() {
        let t = Instant::now();
        let previous_processes: Vec<_> = (0..12)
            .map(|index| ProcessCounter {
                process: process_id(index, 1),
                name: format!("p{index}").into(),
                cpu_time_units: 100,
                rss_bytes: 0,
            })
            .collect();
        let current_processes: Vec<_> = previous_processes
            .iter()
            .map(|process| ProcessCounter {
                cpu_time_units: process.cpu_time_units + u64::from(process.process.pid),
                ..process.clone()
            })
            .collect();
        let previous = RawSnapshot {
            collected_at: t,
            cpu: Collection::available(CpuCounter {
                total_time_units: 1_000,
                idle_time_units: 0,
                logical_cpu_count: 1,
            }),
            processes: Collection::available(previous_processes),
            ..RawSnapshot::default()
        };
        let current = RawSnapshot {
            collected_at: t + Duration::from_secs(1),
            cpu: Collection::available(CpuCounter {
                total_time_units: 1_100,
                idle_time_units: 0,
                logical_cpu_count: 1,
            }),
            processes: Collection::available(current_processes),
            ..RawSnapshot::default()
        };

        let out = derive(Some(&previous), &current);
        let top = out.top_cpu.value().unwrap();
        assert_eq!(top.len(), TOP_N);
        assert_eq!(top[0].name, "p11");
        assert_eq!(top[TOP_N - 1].name, "p4");
        assert_eq!(out.processes.by_id.len(), 12);
    }

    #[test]
    fn derives_process_io_by_pid_and_native_identity() {
        let t = Instant::now();
        let old = RawSnapshot {
            collected_at: t,
            process_disk_io: Collection::available(vec![ProcessDiskIoCounter {
                process: process_id(10, 1),
                disk_id: disk_id("disk-a"),
                device: "old-disk-name".into(),
                read_bytes: 100,
                write_bytes: 200,
            }]),
            process_network_io: Collection::available(vec![ProcessNetworkIoCounter {
                process: process_id(20, 1),
                network_id: network_id("network-a"),
                interface: "old-network-name".into(),
                rx_bytes: 300,
                tx_bytes: 400,
            }]),
            ..RawSnapshot::default()
        };
        let new = RawSnapshot {
            collected_at: t + Duration::from_secs(2),
            processes: Collection::available(vec![
                ProcessCounter {
                    process: process_id(10, 1),
                    name: "disk-worker".into(),
                    cpu_time_units: 0,
                    rss_bytes: 0,
                },
                ProcessCounter {
                    process: process_id(20, 1),
                    name: "network-worker".into(),
                    cpu_time_units: 0,
                    rss_bytes: 0,
                },
            ]),
            process_disk_io: Collection::available(vec![ProcessDiskIoCounter {
                process: process_id(10, 1),
                disk_id: disk_id("disk-a"),
                device: "nvme0n1".into(),
                read_bytes: 500,
                write_bytes: 1000,
            }]),
            process_network_io: Collection::available(vec![ProcessNetworkIoCounter {
                process: process_id(20, 1),
                network_id: network_id("network-a"),
                interface: "eth0".into(),
                rx_bytes: 900,
                tx_bytes: 1400,
            }]),
            ..RawSnapshot::default()
        };

        let out = derive(Some(&old), &new);
        let disk = &out.process_disk_io.value().unwrap()[0];
        assert_eq!(disk.process.pid, 10);
        assert_eq!(disk.name.as_deref(), Some("disk-worker"));
        assert_eq!(disk.device, "nvme0n1");
        assert_eq!(disk.read_bytes_per_sec, 200.0);
        assert_eq!(disk.write_bytes_per_sec, 400.0);
        let network = &out.process_network_io.value().unwrap()[0];
        assert_eq!(network.process.pid, 20);
        assert_eq!(network.name.as_deref(), Some("network-worker"));
        assert_eq!(network.interface, "eth0");
        assert_eq!(network.rx_bytes_per_sec, 300.0);
        assert_eq!(network.tx_bytes_per_sec, 500.0);
    }

    #[test]
    fn process_domain_unions_process_and_attribution_identities() {
        let current = RawSnapshot {
            cpu: Collection::available(CpuCounter {
                total_time_units: 1_000,
                idle_time_units: 500,
                logical_cpu_count: 4,
            }),
            processes: Collection::available(vec![ProcessCounter {
                process: process_id(1, 10),
                name: "known".into(),
                cpu_time_units: 100,
                rss_bytes: 4096,
            }]),
            process_disk_io: Collection::available(vec![ProcessDiskIoCounter {
                process: process_id(2, 20),
                disk_id: disk_id("disk-a"),
                device: "sda".into(),
                read_bytes: 100,
                write_bytes: 200,
            }]),
            process_network_io: Collection::available(vec![ProcessNetworkIoCounter {
                process: process_id(3, 30),
                network_id: network_id("network-a"),
                interface: "eth0".into(),
                rx_bytes: 300,
                tx_bytes: 400,
            }]),
            ..RawSnapshot::default()
        };

        let out = derive(None, &current);

        assert_eq!(out.processes.by_id.len(), 3);
        let known = &out.processes.by_id[&process_id(1, 10)];
        assert_eq!(known.name.as_deref(), Some("known"));
        assert_eq!(known.memory_bytes, Some(4096));
        assert_eq!(known.cpu_percent, Some(0.0));

        let disk_only = &out.processes.by_id[&process_id(2, 20)];
        assert_eq!(disk_only.name, None);
        assert_eq!(disk_only.memory_bytes, None);
        assert_eq!(disk_only.cpu_percent, None);
        assert_eq!(disk_only.disk_io.len(), 1);

        let network_only = &out.processes.by_id[&process_id(3, 30)];
        assert_eq!(network_only.name, None);
        assert_eq!(network_only.network_io.len(), 1);
        assert_eq!(out.processes.metadata_status, CollectionStatus::Available);
        assert_eq!(out.processes.cpu_status, CollectionStatus::Available);
        assert_eq!(out.processes.memory_status, CollectionStatus::Available);
        assert_eq!(out.processes.disk_io_status, CollectionStatus::Available);
        assert_eq!(out.processes.network_io_status, CollectionStatus::Available);
    }

    #[test]
    fn process_io_does_not_reuse_delta_when_display_name_is_reused_by_new_identity() {
        let old_disk = [ProcessDiskIoCounter {
            process: process_id(10, 1),
            disk_id: disk_id("disk-old"),
            device: "sda".into(),
            read_bytes: 100,
            write_bytes: 200,
        }];
        let new_disk = [ProcessDiskIoCounter {
            process: process_id(10, 1),
            disk_id: disk_id("disk-new"),
            device: "sda".into(),
            read_bytes: 500,
            write_bytes: 1000,
        }];
        let old_network = [ProcessNetworkIoCounter {
            process: process_id(20, 1),
            network_id: network_id("network-old"),
            interface: "eth0".into(),
            rx_bytes: 300,
            tx_bytes: 400,
        }];
        let new_network = [ProcessNetworkIoCounter {
            process: process_id(20, 1),
            network_id: network_id("network-new"),
            interface: "eth0".into(),
            rx_bytes: 900,
            tx_bytes: 1400,
        }];

        let disk = derive_process_disk_io_rates(Some(&old_disk), &new_disk, 1.0);
        let network = derive_process_network_io_rates(Some(&old_network), &new_network, 1.0);

        assert_eq!(disk[0].1.read_bytes_per_sec, 0.0);
        assert_eq!(disk[0].1.write_bytes_per_sec, 0.0);
        assert_eq!(network[0].1.rx_bytes_per_sec, 0.0);
        assert_eq!(network[0].1.tx_bytes_per_sec, 0.0);
    }

    #[test]
    fn process_domain_keeps_complete_io_before_card_top_n_projection() {
        let t = Instant::now();
        let mut old_disk: Vec<_> = (1..=4)
            .map(|pid| ProcessDiskIoCounter {
                process: process_id(pid, 1),
                disk_id: disk_id("disk-a"),
                device: "sda".into(),
                read_bytes: 0,
                write_bytes: 0,
            })
            .collect();
        old_disk.push(ProcessDiskIoCounter {
            process: process_id(5, 1),
            disk_id: disk_id("disk-b"),
            device: "sdb".into(),
            read_bytes: 0,
            write_bytes: 0,
        });
        let mut new_disk: Vec<_> = (1..=4)
            .map(|pid| ProcessDiskIoCounter {
                process: process_id(pid, 1),
                disk_id: disk_id("disk-a"),
                device: "sda".into(),
                read_bytes: u64::from(pid) * 100,
                write_bytes: u64::from(pid) * 10,
            })
            .collect();
        new_disk.push(ProcessDiskIoCounter {
            process: process_id(5, 1),
            disk_id: disk_id("disk-b"),
            device: "sdb".into(),
            read_bytes: 1,
            write_bytes: 0,
        });

        let mut old_network: Vec<_> = (1..=4)
            .map(|pid| ProcessNetworkIoCounter {
                process: process_id(pid, 1),
                network_id: network_id("network-a"),
                interface: "eth0".into(),
                rx_bytes: 0,
                tx_bytes: 0,
            })
            .collect();
        old_network.push(ProcessNetworkIoCounter {
            process: process_id(5, 1),
            network_id: network_id("network-b"),
            interface: "eth1".into(),
            rx_bytes: 0,
            tx_bytes: 0,
        });
        let mut new_network: Vec<_> = (1..=4)
            .map(|pid| ProcessNetworkIoCounter {
                process: process_id(pid, 1),
                network_id: network_id("network-a"),
                interface: "eth0".into(),
                rx_bytes: u64::from(pid) * 100,
                tx_bytes: u64::from(pid) * 10,
            })
            .collect();
        new_network.push(ProcessNetworkIoCounter {
            process: process_id(5, 1),
            network_id: network_id("network-b"),
            interface: "eth1".into(),
            rx_bytes: 1,
            tx_bytes: 0,
        });
        let processes = (1..=5)
            .map(|pid| ProcessCounter {
                process: process_id(pid, 1),
                name: format!("p{pid}").into(),
                cpu_time_units: 0,
                rss_bytes: u64::from(pid),
            })
            .collect();
        let old = RawSnapshot {
            collected_at: t,
            process_disk_io: Collection::available(old_disk),
            process_network_io: Collection::available(old_network),
            ..RawSnapshot::default()
        };
        let new = RawSnapshot {
            collected_at: t + Duration::from_secs(1),
            processes: Collection::available(processes),
            process_disk_io: Collection::available(new_disk),
            process_network_io: Collection::available(new_network),
            ..RawSnapshot::default()
        };

        let out = derive(Some(&old), &new);
        let disk = out.process_disk_io.value().unwrap();
        let network = out.process_network_io.value().unwrap();

        assert_eq!(disk.len(), PROCESS_IO_TOP_N + 1);
        assert_eq!(network.len(), PROCESS_IO_TOP_N + 1);
        assert_eq!(
            disk.iter()
                .filter(|row| row.disk_id == disk_id("disk-a"))
                .map(|row| row.process.pid)
                .collect::<Vec<_>>(),
            [4, 3, 2]
        );
        assert_eq!(
            network
                .iter()
                .filter(|row| row.network_id == network_id("network-a"))
                .map(|row| row.process.pid)
                .collect::<Vec<_>>(),
            [4, 3, 2]
        );
        assert_eq!(out.processes.by_id.len(), 5);
        assert_eq!(out.processes.by_id[&process_id(1, 1)].disk_io.len(), 1);
        assert_eq!(out.processes.by_id[&process_id(1, 1)].network_io.len(), 1);
        assert_eq!(disk[0].name.as_deref(), Some("p4"));
        assert_eq!(network[0].name.as_deref(), Some("p4"));
    }

    #[test]
    fn attribution_counter_reset_does_not_create_a_rate_spike() {
        let t = Instant::now();
        let old = RawSnapshot {
            collected_at: t,
            process_disk_io: Collection::available(vec![ProcessDiskIoCounter {
                process: process_id(10, 1),
                disk_id: disk_id("disk-a"),
                device: "nvme0n1".into(),
                read_bytes: 10_000,
                write_bytes: 20_000,
            }]),
            process_network_io: Collection::available(vec![ProcessNetworkIoCounter {
                process: process_id(20, 1),
                network_id: network_id("network-a"),
                interface: "eth0".into(),
                rx_bytes: 30_000,
                tx_bytes: 40_000,
            }]),
            ..RawSnapshot::default()
        };
        let new = RawSnapshot {
            collected_at: t + Duration::from_secs(1),
            process_disk_io: Collection::available(vec![ProcessDiskIoCounter {
                process: process_id(10, 1),
                disk_id: disk_id("disk-a"),
                device: "nvme0n1".into(),
                read_bytes: 5,
                write_bytes: 7,
            }]),
            process_network_io: Collection::available(vec![ProcessNetworkIoCounter {
                process: process_id(20, 1),
                network_id: network_id("network-a"),
                interface: "eth0".into(),
                rx_bytes: 11,
                tx_bytes: 13,
            }]),
            ..RawSnapshot::default()
        };

        let out = derive(Some(&old), &new);
        let disk = &out.process_disk_io.value().unwrap()[0];
        assert_eq!(
            (disk.read_bytes_per_sec, disk.write_bytes_per_sec),
            (0.0, 0.0)
        );
        let network = &out.process_network_io.value().unwrap()[0];
        assert_eq!(
            (network.rx_bytes_per_sec, network.tx_bytes_per_sec),
            (0.0, 0.0)
        );
    }

    #[test]
    fn pid_reuse_does_not_inherit_cpu_delta() {
        let t = Instant::now();
        let previous = RawSnapshot {
            collected_at: t,
            cpu: Collection::available(CpuCounter {
                total_time_units: 1_000,
                idle_time_units: 0,
                logical_cpu_count: 1,
            }),
            processes: Collection::available(vec![ProcessCounter {
                process: process_id(42, 100),
                name: "old".into(),
                cpu_time_units: 1_000,
                rss_bytes: 0,
            }]),
            ..RawSnapshot::default()
        };
        let current = RawSnapshot {
            collected_at: t + Duration::from_secs(1),
            cpu: Collection::available(CpuCounter {
                total_time_units: 1_100,
                idle_time_units: 0,
                logical_cpu_count: 1,
            }),
            processes: Collection::available(vec![ProcessCounter {
                process: process_id(42, 200),
                name: "new".into(),
                cpu_time_units: 25,
                rss_bytes: 0,
            }]),
            ..RawSnapshot::default()
        };

        let out = derive(Some(&previous), &current);
        let process = &out.processes.by_id[&process_id(42, 200)];
        assert_eq!(process.name.as_deref(), Some("new"));
        assert_eq!(process.cpu_percent, Some(0.0));
    }

    #[test]
    fn pid_reuse_does_not_inherit_process_io_delta() {
        let old_disk = [ProcessDiskIoCounter {
            process: process_id(42, 100),
            disk_id: disk_id("disk-a"),
            device: "sda".into(),
            read_bytes: 10_000,
            write_bytes: 20_000,
        }];
        let new_disk = [ProcessDiskIoCounter {
            process: process_id(42, 200),
            disk_id: disk_id("disk-a"),
            device: "sda".into(),
            read_bytes: 100,
            write_bytes: 200,
        }];
        let old_network = [ProcessNetworkIoCounter {
            process: process_id(42, 100),
            network_id: network_id("network-a"),
            interface: "eth0".into(),
            rx_bytes: 30_000,
            tx_bytes: 40_000,
        }];
        let new_network = [ProcessNetworkIoCounter {
            process: process_id(42, 200),
            network_id: network_id("network-a"),
            interface: "eth0".into(),
            rx_bytes: 300,
            tx_bytes: 400,
        }];

        let disk = derive_process_disk_io_rates(Some(&old_disk), &new_disk, 1.0);
        let network = derive_process_network_io_rates(Some(&old_network), &new_network, 1.0);

        assert_eq!(disk[0].0, process_id(42, 200));
        assert_eq!(
            (disk[0].1.read_bytes_per_sec, disk[0].1.write_bytes_per_sec),
            (0.0, 0.0)
        );
        assert_eq!(network[0].0, process_id(42, 200));
        assert_eq!(
            (network[0].1.rx_bytes_per_sec, network[0].1.tx_bytes_per_sec),
            (0.0, 0.0)
        );
    }

    #[test]
    fn preserves_attribution_unavailability() {
        let t = Instant::now();
        let old = RawSnapshot {
            collected_at: t,
            process_disk_io: Collection::available(Vec::new()),
            process_network_io: Collection::available(Vec::new()),
            ..RawSnapshot::default()
        };
        let new = RawSnapshot {
            collected_at: t + Duration::from_secs(1),
            process_disk_io: Collection::unavailable(CollectionUnavailable::PermissionDenied),
            process_network_io: Collection::unavailable(CollectionUnavailable::PermissionDenied),
            ..RawSnapshot::default()
        };

        let out = derive(Some(&old), &new);
        assert_eq!(
            out.process_disk_io.status(),
            CollectionStatus::Unavailable(CollectionUnavailable::PermissionDenied)
        );
        assert_eq!(
            out.process_network_io.status(),
            CollectionStatus::Unavailable(CollectionUnavailable::PermissionDenied)
        );
        assert_eq!(
            out.processes.disk_io_status,
            CollectionStatus::Unavailable(CollectionUnavailable::PermissionDenied)
        );
        assert_eq!(
            out.processes.network_io_status,
            CollectionStatus::Unavailable(CollectionUnavailable::PermissionDenied)
        );
    }

    #[test]
    fn derivation_preserves_degraded_observation_status() {
        let current = RawSnapshot {
            networks: Collection::degraded(vec![NetworkCounter {
                id: network_id("network-a"),
                name: "eth0".into(),
                rx_bytes: 100,
                tx_bytes: 200,
            }]),
            process_disk_io: Collection::degraded(Vec::new()),
            ..RawSnapshot::default()
        };

        let out = derive(None, &current);

        assert_eq!(out.networks.status(), CollectionStatus::Degraded);
        assert_eq!(out.process_disk_io.status(), CollectionStatus::Degraded);
        assert_eq!(out.processes.disk_io_status, CollectionStatus::Degraded);
        assert_eq!(out.networks.value().unwrap().len(), 1);
        assert!(out.process_disk_io.value().unwrap().is_empty());
    }
}
