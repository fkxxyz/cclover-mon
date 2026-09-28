use std::collections::{BTreeMap, HashMap};
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
    let process_names: HashMap<ProcessInstanceId, &str> = current_processes
        .iter()
        .map(|process| (process.process, process.name.as_str()))
        .collect();
    let (cpu_percent, top_cpu) = derive_cpu(previous, current, current_processes);
    let dt = sample_interval_seconds(previous, current);
    let mut out = SystemSnapshot {
        cpu_percent,
        memory: current.memory.value().cloned(),
        top_cpu,
        top_memory: top_memory(current_processes),
        networks: derive_networks(previous, current, dt),
        disks: derive_disks(previous, current, dt),
        temperatures: current.temperatures.value().cloned().unwrap_or_default(),
        ..SystemSnapshot::default()
    };

    out.process_disk_io = derive_process_disk_io(
        previous.and_then(|snapshot| snapshot.process_disk_io.value().map(Vec::as_slice)),
        current.process_disk_io.value().map(Vec::as_slice),
        &process_names,
        dt,
    );
    out.process_network_io = derive_process_network_io(
        previous.and_then(|snapshot| snapshot.process_network_io.value().map(Vec::as_slice)),
        current.process_network_io.value().map(Vec::as_slice),
        &process_names,
        dt,
    );

    out
}

fn derive_cpu(
    previous: Option<&RawSnapshot>,
    current: &RawSnapshot,
    current_processes: &[ProcessCounter],
) -> (Option<f64>, Vec<ProcessCpuUsage>) {
    let Some(new) = current.cpu.value() else {
        return (None, Vec::new());
    };
    let Some((previous, old)) =
        previous.and_then(|snapshot| snapshot.cpu.value().map(|counter| (snapshot, counter)))
    else {
        return (Some(0.0), baseline_top_cpu(current_processes));
    };

    let total = new.total_time_units.saturating_sub(old.total_time_units);
    if total == 0 {
        return (Some(0.0), baseline_top_cpu(current_processes));
    }

    let idle = new.idle_time_units.saturating_sub(old.idle_time_units);
    let percent = (100.0 * (total.saturating_sub(idle)) as f64 / total as f64).clamp(0.0, 100.0);
    let top_cpu = previous.processes.value().map_or_else(
        || baseline_top_cpu(current_processes),
        |previous_processes| {
            top_cpu(
                previous_processes,
                current_processes,
                total,
                new.logical_cpu_count,
            )
        },
    );
    (Some(percent), top_cpu)
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
) -> Vec<NetworkSnapshot> {
    let old: HashMap<&NetworkId, &NetworkCounter> = previous
        .and_then(|snapshot| snapshot.networks.value())
        .into_iter()
        .flatten()
        .map(|item| (&item.id, item))
        .collect();
    current
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
        .collect()
}

fn derive_disks(
    previous: Option<&RawSnapshot>,
    current: &RawSnapshot,
    dt: f64,
) -> Vec<DiskSnapshot> {
    let old: HashMap<&DiskId, &DiskCounter> = previous
        .and_then(|snapshot| snapshot.disks.value())
        .into_iter()
        .flatten()
        .map(|item| (&item.id, item))
        .collect();
    current
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
        .collect()
}

fn derive_process_disk_io(
    previous: Option<&[ProcessDiskIoCounter]>,
    current: Option<&[ProcessDiskIoCounter]>,
    process_names: &HashMap<ProcessInstanceId, &str>,
    dt: f64,
) -> Option<Vec<ProcessDiskIo>> {
    let current = current?;
    let old: HashMap<(ProcessInstanceId, &DiskId), &ProcessDiskIoCounter> = previous
        .into_iter()
        .flatten()
        .map(|item| ((item.process, &item.disk_id), item))
        .collect();
    let mut by_disk: BTreeMap<DiskId, Vec<ProcessDiskIo>> = BTreeMap::new();
    for item in current {
        let (read, write) = old
            .get(&(item.process, &item.disk_id))
            .map_or((0.0, 0.0), |old| {
                (
                    item.read_bytes.saturating_sub(old.read_bytes) as f64 / dt,
                    item.write_bytes.saturating_sub(old.write_bytes) as f64 / dt,
                )
            });
        by_disk
            .entry(item.disk_id.clone())
            .or_default()
            .push(ProcessDiskIo {
                process: item.process,
                name: process_names
                    .get(&item.process)
                    .map(|name| (*name).to_owned()),
                disk_id: item.disk_id.clone(),
                device: item.device.clone(),
                read_bytes_per_sec: read,
                write_bytes_per_sec: write,
            });
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
    Some(by_disk.into_values().flatten().collect())
}

fn derive_process_network_io(
    previous: Option<&[ProcessNetworkIoCounter]>,
    current: Option<&[ProcessNetworkIoCounter]>,
    process_names: &HashMap<ProcessInstanceId, &str>,
    dt: f64,
) -> Option<Vec<ProcessNetworkIo>> {
    let current = current?;
    let old: HashMap<(ProcessInstanceId, &NetworkId), &ProcessNetworkIoCounter> = previous
        .into_iter()
        .flatten()
        .map(|item| ((item.process, &item.network_id), item))
        .collect();
    let mut by_network: BTreeMap<NetworkId, Vec<ProcessNetworkIo>> = BTreeMap::new();
    for item in current {
        let (rx, tx) = old
            .get(&(item.process, &item.network_id))
            .map_or((0.0, 0.0), |old| {
                (
                    item.rx_bytes.saturating_sub(old.rx_bytes) as f64 / dt,
                    item.tx_bytes.saturating_sub(old.tx_bytes) as f64 / dt,
                )
            });
        by_network
            .entry(item.network_id.clone())
            .or_default()
            .push(ProcessNetworkIo {
                process: item.process,
                name: process_names
                    .get(&item.process)
                    .map(|name| (*name).to_owned()),
                network_id: item.network_id.clone(),
                interface: item.interface.clone(),
                rx_bytes_per_sec: rx,
                tx_bytes_per_sec: tx,
            });
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
    Some(by_network.into_values().flatten().collect())
}

fn baseline_top_cpu(current: &[ProcessCounter]) -> Vec<ProcessCpuUsage> {
    current
        .iter()
        .take(TOP_N)
        .map(|item| ProcessCpuUsage {
            name: item.name.clone(),
            percent: 0.0,
        })
        .collect()
}

fn top_cpu(
    previous: &[ProcessCounter],
    current: &[ProcessCounter],
    total_delta: u64,
    cpu_count: usize,
) -> Vec<ProcessCpuUsage> {
    let old: HashMap<ProcessInstanceId, &ProcessCounter> =
        previous.iter().map(|x| (x.process, x)).collect();
    let scale = cpu_count.max(1) as f64 * 100.0 / total_delta.max(1) as f64;
    let mut values: Vec<_> = current
        .iter()
        .map(|item| {
            let delta = old.get(&item.process).map_or(0, |prev| {
                item.cpu_time_units.saturating_sub(prev.cpu_time_units)
            });
            (item, delta as f64 * scale)
        })
        .collect();
    keep_top_n_by(&mut values, TOP_N, |a, b| b.1.total_cmp(&a.1));
    values
        .into_iter()
        .map(|(item, percent)| ProcessCpuUsage {
            name: item.name.clone(),
            percent,
        })
        .collect()
}

fn top_memory(current: &[ProcessCounter]) -> Vec<ProcessMemoryUsage> {
    let mut values: Vec<_> = current.iter().collect();
    keep_top_n_by(&mut values, TOP_N, |a, b| b.rss_bytes.cmp(&a.rss_bytes));
    values
        .into_iter()
        .map(|item| ProcessMemoryUsage {
            name: item.name.clone(),
            bytes: item.rss_bytes,
        })
        .collect()
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

        assert_eq!(out.cpu_percent, Some(0.0));
        assert_eq!(out.top_cpu.len(), 1);
        assert_eq!(out.top_cpu[0].name, "worker");
        assert_eq!(out.top_cpu[0].percent, 0.0);
        assert_eq!(out.top_memory[0].name, "worker");
        assert_eq!(out.memory.as_ref().unwrap().used_bytes, 10);
        assert_eq!(out.temperatures[0].name, "CPU");
        assert_eq!(out.networks[0].name, "eth0");
        assert_eq!(
            (
                out.networks[0].down_bytes_per_sec,
                out.networks[0].up_bytes_per_sec,
            ),
            (0.0, 0.0)
        );
        assert_eq!(out.disks[0].name, "nvme0n1");
        assert_eq!(out.disks[0].bytes_per_sec, 0.0);

        let disk = out.process_disk_io.as_ref().unwrap();
        assert_eq!(disk[0].device, "nvme0n1");
        assert_eq!(
            (disk[0].read_bytes_per_sec, disk[0].write_bytes_per_sec),
            (0.0, 0.0)
        );
        let network = out.process_network_io.as_ref().unwrap();
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
        assert_eq!(out.cpu_percent, Some(50.0));
        assert_eq!(out.networks[0].down_bytes_per_sec, 200.0);
        assert_eq!(out.networks[0].up_bytes_per_sec, 300.0);
        assert_eq!(out.disks[0].bytes_per_sec, 600.0);
        assert!((out.top_cpu[0].percent - 40.0).abs() < 0.001);
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

        assert_eq!(out.networks[0].name, "lan0");
        assert_eq!(out.networks[0].down_bytes_per_sec, 200.0);
        assert_eq!(out.networks[0].up_bytes_per_sec, 300.0);
        assert_eq!(out.disks[0].name, "system-disk");
        assert_eq!(out.disks[0].bytes_per_sec, 600.0);
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

        assert_eq!(out.networks[0].down_bytes_per_sec, 0.0);
        assert_eq!(out.networks[0].up_bytes_per_sec, 0.0);
        assert_eq!(out.disks[0].bytes_per_sec, 0.0);
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

        assert_eq!(out.networks[0].down_bytes_per_sec, 0.0);
        assert_eq!(out.networks[0].up_bytes_per_sec, 0.0);
        assert_eq!(out.disks[0].bytes_per_sec, 0.0);
    }

    #[test]
    fn top_memory_keeps_only_highest_processes_in_order() {
        let processes: Vec<_> = (0..12)
            .map(|index| ProcessCounter {
                process: process_id(index, 1),
                name: format!("p{index}"),
                cpu_time_units: 0,
                rss_bytes: u64::from(index),
            })
            .collect();

        let top = top_memory(&processes);
        assert_eq!(top.len(), TOP_N);
        assert_eq!(top[0].name, "p11");
        assert_eq!(top[TOP_N - 1].name, "p4");
    }

    #[test]
    fn top_cpu_keeps_only_highest_processes_in_order() {
        let previous: Vec<_> = (0..12)
            .map(|index| ProcessCounter {
                process: process_id(index, 1),
                name: format!("p{index}"),
                cpu_time_units: 100,
                rss_bytes: 0,
            })
            .collect();
        let current: Vec<_> = previous
            .iter()
            .map(|process| ProcessCounter {
                cpu_time_units: process.cpu_time_units + u64::from(process.process.pid),
                ..process.clone()
            })
            .collect();

        let top = top_cpu(&previous, &current, 100, 1);
        assert_eq!(top.len(), TOP_N);
        assert_eq!(top[0].name, "p11");
        assert_eq!(top[TOP_N - 1].name, "p4");
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
        let disk = &out.process_disk_io.unwrap()[0];
        assert_eq!(disk.process.pid, 10);
        assert_eq!(disk.name.as_deref(), Some("disk-worker"));
        assert_eq!(disk.device, "nvme0n1");
        assert_eq!(disk.read_bytes_per_sec, 200.0);
        assert_eq!(disk.write_bytes_per_sec, 400.0);
        let network = &out.process_network_io.unwrap()[0];
        assert_eq!(network.process.pid, 20);
        assert_eq!(network.name.as_deref(), Some("network-worker"));
        assert_eq!(network.interface, "eth0");
        assert_eq!(network.rx_bytes_per_sec, 300.0);
        assert_eq!(network.tx_bytes_per_sec, 500.0);
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

        let disk =
            derive_process_disk_io(Some(&old_disk), Some(&new_disk), &HashMap::new(), 1.0).unwrap();
        let network =
            derive_process_network_io(Some(&old_network), Some(&new_network), &HashMap::new(), 1.0)
                .unwrap();

        assert_eq!(disk[0].read_bytes_per_sec, 0.0);
        assert_eq!(disk[0].write_bytes_per_sec, 0.0);
        assert_eq!(network[0].rx_bytes_per_sec, 0.0);
        assert_eq!(network[0].tx_bytes_per_sec, 0.0);
    }

    #[test]
    fn process_io_keeps_top_three_per_device() {
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

        let process_names: HashMap<_, _> = (1..=5)
            .map(|pid| {
                (
                    process_id(pid, 1),
                    match pid {
                        1 => "p1",
                        2 => "p2",
                        3 => "p3",
                        4 => "p4",
                        _ => "p5",
                    },
                )
            })
            .collect();

        let disk =
            derive_process_disk_io(Some(&old_disk), Some(&new_disk), &process_names, 1.0).unwrap();
        let network =
            derive_process_network_io(Some(&old_network), Some(&new_network), &process_names, 1.0)
                .unwrap();

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
        assert!(disk.iter().any(|row| row.process.pid == 5));
        assert!(network.iter().any(|row| row.process.pid == 5));
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
        let disk = &out.process_disk_io.unwrap()[0];
        assert_eq!(
            (disk.read_bytes_per_sec, disk.write_bytes_per_sec),
            (0.0, 0.0)
        );
        let network = &out.process_network_io.unwrap()[0];
        assert_eq!(
            (network.rx_bytes_per_sec, network.tx_bytes_per_sec),
            (0.0, 0.0)
        );
    }

    #[test]
    fn pid_reuse_does_not_inherit_cpu_delta() {
        let previous = vec![ProcessCounter {
            process: process_id(42, 100),
            name: "old".into(),
            cpu_time_units: 1_000,
            rss_bytes: 0,
        }];
        let current = vec![ProcessCounter {
            process: process_id(42, 200),
            name: "new".into(),
            cpu_time_units: 25,
            rss_bytes: 0,
        }];

        let top = top_cpu(&previous, &current, 100, 1);
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].name, "new");
        assert_eq!(top[0].percent, 0.0);
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

        let disk =
            derive_process_disk_io(Some(&old_disk), Some(&new_disk), &HashMap::new(), 1.0).unwrap();
        let network =
            derive_process_network_io(Some(&old_network), Some(&new_network), &HashMap::new(), 1.0)
                .unwrap();

        assert_eq!(disk[0].process, process_id(42, 200));
        assert_eq!(
            (disk[0].read_bytes_per_sec, disk[0].write_bytes_per_sec),
            (0.0, 0.0)
        );
        assert_eq!(network[0].process, process_id(42, 200));
        assert_eq!(
            (network[0].rx_bytes_per_sec, network[0].tx_bytes_per_sec),
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
            process_disk_io: Collection::unavailable(CollectionUnavailable::Unavailable),
            process_network_io: Collection::unavailable(CollectionUnavailable::Unavailable),
            ..RawSnapshot::default()
        };

        let out = derive(Some(&old), &new);
        assert!(out.process_disk_io.is_none());
        assert!(out.process_network_io.is_none());
    }
}
