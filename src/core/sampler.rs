use std::collections::HashMap;

use super::history;
use super::model::*;
use crate::platform::Collector;

const TOP_N: usize = 8;
const HISTORY_CAPACITY: usize = 60;

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
        let raw = self.collector.collect();
        let snapshot = derive(self.previous.as_ref(), &raw);
        history::push(
            &mut self.state.history,
            &snapshot,
            self.state.history_capacity,
        );
        self.state.snapshot = snapshot;
        self.previous = Some(raw);
        self.state.clone()
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

    out
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
            ProcessCpu {
                name: item.name.clone(),
                percent: delta as f64 * scale,
            }
        })
        .collect();
    values.sort_unstable_by(|a, b| b.percent.total_cmp(&a.percent));
    values.truncate(TOP_N);
    values
}

fn top_memory(current: &[ProcessCounter]) -> Vec<ProcessMemory> {
    let mut values: Vec<_> = current
        .iter()
        .map(|item| ProcessMemory {
            name: item.name.clone(),
            bytes: item.rss_bytes,
        })
        .collect();
    values.sort_unstable_by_key(|item| std::cmp::Reverse(item.bytes));
    values.truncate(TOP_N);
    values
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
}
