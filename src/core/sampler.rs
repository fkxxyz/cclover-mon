use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::devlog;
use super::history;
use super::model::*;

const TOP_N: usize = 8;
const HISTORY_CAPACITY: usize = 60;
pub const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);

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
    let mut out = SystemSnapshot {
        memory: current.memory.clone(),
        temperatures: current.temperatures.clone(),
        ..SystemSnapshot::default()
    };

    let Some(previous) = previous else {
        out.top_memory = top_memory(&current.processes);
        return out;
    };

    if let (Some(old), Some(new)) = (&previous.cpu, &current.cpu) {
        let total = new.total_jiffies.saturating_sub(old.total_jiffies);
        let idle = new.idle_jiffies.saturating_sub(old.idle_jiffies);
        if total > 0 {
            out.cpu_percent = Some(
                (100.0 * (total.saturating_sub(idle)) as f64 / total as f64).clamp(0.0, 100.0),
            );
            out.top_cpu = top_cpu(
                &previous.processes,
                &current.processes,
                total,
                new.logical_cpu_count,
            );
        }
    }
    out.top_memory = top_memory(&current.processes);

    let dt = current
        .collected_at
        .saturating_duration_since(previous.collected_at)
        .as_secs_f64()
        .max(0.001);

    let old_net: HashMap<&str, &NetworkCounter> = previous
        .networks
        .iter()
        .map(|x| (x.name.as_str(), x))
        .collect();
    out.networks = current
        .networks
        .iter()
        .map(|item| {
            let (down, up) = old_net.get(item.name.as_str()).map_or((0.0, 0.0), |old| {
                (
                    item.rx_bytes.saturating_sub(old.rx_bytes) as f64 / dt,
                    item.tx_bytes.saturating_sub(old.tx_bytes) as f64 / dt,
                )
            });
            NetworkSnapshot {
                name: item.name.clone(),
                down_bytes_per_sec: down,
                up_bytes_per_sec: up,
            }
        })
        .collect();

    let old_disk: HashMap<&str, &DiskCounter> = previous
        .disks
        .iter()
        .map(|x| (x.name.as_str(), x))
        .collect();
    out.disks = current
        .disks
        .iter()
        .map(|item| {
            let rate = old_disk.get(item.name.as_str()).map_or(0.0, |old| {
                let old_total = old.read_bytes.saturating_add(old.write_bytes);
                let new_total = item.read_bytes.saturating_add(item.write_bytes);
                new_total.saturating_sub(old_total) as f64 / dt
            });
            DiskSnapshot {
                name: item.name.clone(),
                bytes_per_sec: rate,
            }
        })
        .collect();

    out.process_disk_io = derive_process_disk_io(
        previous.process_disk_io.as_deref(),
        current.process_disk_io.as_deref(),
        dt,
    );
    out.process_network_io = derive_process_network_io(
        previous.process_network_io.as_deref(),
        current.process_network_io.as_deref(),
        dt,
    );

    out
}

fn derive_process_disk_io(
    previous: Option<&[ProcessDiskIoCounter]>,
    current: Option<&[ProcessDiskIoCounter]>,
    dt: f64,
) -> Option<Vec<ProcessDiskIo>> {
    let current = current?;
    let previous = previous?;
    let old: HashMap<(u32, &str), &ProcessDiskIoCounter> = previous
        .iter()
        .map(|item| ((item.pid, item.device.as_str()), item))
        .collect();
    Some(
        current
            .iter()
            .map(|item| {
                let (read, write) =
                    old.get(&(item.pid, item.device.as_str()))
                        .map_or((0.0, 0.0), |old| {
                            (
                                item.read_bytes.saturating_sub(old.read_bytes) as f64 / dt,
                                item.write_bytes.saturating_sub(old.write_bytes) as f64 / dt,
                            )
                        });
                ProcessDiskIo {
                    pid: item.pid,
                    device: item.device.clone(),
                    read_bytes_per_sec: read,
                    write_bytes_per_sec: write,
                }
            })
            .collect(),
    )
}

fn derive_process_network_io(
    previous: Option<&[ProcessNetworkIoCounter]>,
    current: Option<&[ProcessNetworkIoCounter]>,
    dt: f64,
) -> Option<Vec<ProcessNetworkIo>> {
    let current = current?;
    let previous = previous?;
    let old: HashMap<(u32, &str), &ProcessNetworkIoCounter> = previous
        .iter()
        .map(|item| ((item.pid, item.interface.as_str()), item))
        .collect();
    Some(
        current
            .iter()
            .map(|item| {
                let (rx, tx) =
                    old.get(&(item.pid, item.interface.as_str()))
                        .map_or((0.0, 0.0), |old| {
                            (
                                item.rx_bytes.saturating_sub(old.rx_bytes) as f64 / dt,
                                item.tx_bytes.saturating_sub(old.tx_bytes) as f64 / dt,
                            )
                        });
                ProcessNetworkIo {
                    pid: item.pid,
                    interface: item.interface.clone(),
                    rx_bytes_per_sec: rx,
                    tx_bytes_per_sec: tx,
                }
            })
            .collect(),
    )
}

fn top_cpu(
    previous: &[ProcessCounter],
    current: &[ProcessCounter],
    total_delta: u64,
    cpu_count: usize,
) -> Vec<ProcessCpu> {
    let old: HashMap<u32, &ProcessCounter> = previous.iter().map(|x| (x.pid, x)).collect();
    let scale = cpu_count.max(1) as f64 * 100.0 / total_delta.max(1) as f64;
    let mut values: Vec<_> = current
        .iter()
        .map(|item| {
            let delta = old
                .get(&item.pid)
                .map_or(0, |prev| item.cpu_ticks.saturating_sub(prev.cpu_ticks));
            (item, delta as f64 * scale)
        })
        .collect();
    keep_top_n_by(&mut values, TOP_N, |a, b| b.1.total_cmp(&a.1));
    values
        .into_iter()
        .map(|(item, percent)| ProcessCpu {
            name: item.name.clone(),
            percent,
        })
        .collect()
}

fn top_memory(current: &[ProcessCounter]) -> Vec<ProcessMemory> {
    let mut values: Vec<_> = current.iter().collect();
    keep_top_n_by(&mut values, TOP_N, |a, b| b.rss_bytes.cmp(&a.rss_bytes));
    values
        .into_iter()
        .map(|item| ProcessMemory {
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

    #[test]
    fn derives_rates_and_cpu() {
        let t = Instant::now();
        let old = RawSnapshot {
            collected_at: t,
            cpu: Some(CpuCounter {
                total_jiffies: 1000,
                idle_jiffies: 600,
                logical_cpu_count: 4,
            }),
            processes: vec![ProcessCounter {
                pid: 1,
                name: "a".into(),
                cpu_ticks: 100,
                rss_bytes: 10,
            }],
            networks: vec![NetworkCounter {
                name: "eth0".into(),
                rx_bytes: 100,
                tx_bytes: 200,
            }],
            disks: vec![DiskCounter {
                name: "sda".into(),
                read_bytes: 100,
                write_bytes: 100,
            }],
            ..RawSnapshot::default()
        };
        let new = RawSnapshot {
            collected_at: t + Duration::from_secs(1),
            cpu: Some(CpuCounter {
                total_jiffies: 1100,
                idle_jiffies: 650,
                logical_cpu_count: 4,
            }),
            processes: vec![ProcessCounter {
                pid: 1,
                name: "a".into(),
                cpu_ticks: 110,
                rss_bytes: 20,
            }],
            networks: vec![NetworkCounter {
                name: "eth0".into(),
                rx_bytes: 300,
                tx_bytes: 500,
            }],
            disks: vec![DiskCounter {
                name: "sda".into(),
                read_bytes: 300,
                write_bytes: 500,
            }],
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
    fn top_memory_keeps_only_highest_processes_in_order() {
        let processes: Vec<_> = (0..12)
            .map(|index| ProcessCounter {
                pid: index,
                name: format!("p{index}"),
                cpu_ticks: 0,
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
                pid: index,
                name: format!("p{index}"),
                cpu_ticks: 100,
                rss_bytes: 0,
            })
            .collect();
        let current: Vec<_> = previous
            .iter()
            .map(|process| ProcessCounter {
                cpu_ticks: process.cpu_ticks + u64::from(process.pid),
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
            process_disk_io: Some(vec![ProcessDiskIoCounter {
                pid: 10,
                device: "nvme0n1".into(),
                read_bytes: 100,
                write_bytes: 200,
            }]),
            process_network_io: Some(vec![ProcessNetworkIoCounter {
                pid: 20,
                interface: "eth0".into(),
                rx_bytes: 300,
                tx_bytes: 400,
            }]),
            ..RawSnapshot::default()
        };
        let new = RawSnapshot {
            collected_at: t + Duration::from_secs(2),
            process_disk_io: Some(vec![ProcessDiskIoCounter {
                pid: 10,
                device: "nvme0n1".into(),
                read_bytes: 500,
                write_bytes: 1000,
            }]),
            process_network_io: Some(vec![ProcessNetworkIoCounter {
                pid: 20,
                interface: "eth0".into(),
                rx_bytes: 900,
                tx_bytes: 1400,
            }]),
            ..RawSnapshot::default()
        };

        let out = derive(Some(&old), &new);
        let disk = &out.process_disk_io.unwrap()[0];
        assert_eq!(disk.pid, 10);
        assert_eq!(disk.device, "nvme0n1");
        assert_eq!(disk.read_bytes_per_sec, 200.0);
        assert_eq!(disk.write_bytes_per_sec, 400.0);
        let network = &out.process_network_io.unwrap()[0];
        assert_eq!(network.pid, 20);
        assert_eq!(network.interface, "eth0");
        assert_eq!(network.rx_bytes_per_sec, 300.0);
        assert_eq!(network.tx_bytes_per_sec, 500.0);
    }

    #[test]
    fn attribution_counter_reset_does_not_create_a_rate_spike() {
        let t = Instant::now();
        let old = RawSnapshot {
            collected_at: t,
            process_disk_io: Some(vec![ProcessDiskIoCounter {
                pid: 10,
                device: "nvme0n1".into(),
                read_bytes: 10_000,
                write_bytes: 20_000,
            }]),
            process_network_io: Some(vec![ProcessNetworkIoCounter {
                pid: 20,
                interface: "eth0".into(),
                rx_bytes: 30_000,
                tx_bytes: 40_000,
            }]),
            ..RawSnapshot::default()
        };
        let new = RawSnapshot {
            collected_at: t + Duration::from_secs(1),
            process_disk_io: Some(vec![ProcessDiskIoCounter {
                pid: 10,
                device: "nvme0n1".into(),
                read_bytes: 5,
                write_bytes: 7,
            }]),
            process_network_io: Some(vec![ProcessNetworkIoCounter {
                pid: 20,
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
    fn preserves_attribution_unavailability() {
        let t = Instant::now();
        let old = RawSnapshot {
            collected_at: t,
            process_disk_io: Some(Vec::new()),
            process_network_io: Some(Vec::new()),
            ..RawSnapshot::default()
        };
        let new = RawSnapshot {
            collected_at: t + Duration::from_secs(1),
            process_disk_io: None,
            process_network_io: None,
            ..RawSnapshot::default()
        };

        let out = derive(Some(&old), &new);
        assert!(out.process_disk_io.is_none());
        assert!(out.process_network_io.is_none());
    }
}
