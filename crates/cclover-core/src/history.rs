use std::collections::{BTreeMap, VecDeque};

use super::model::{Collection, MonitorHistory, SystemSnapshot};

pub fn push(history: &mut MonitorHistory, snapshot: &SystemSnapshot, capacity: usize) {
    if let Some(cpu) = snapshot.cpu_percent.value() {
        append(&mut history.cpu, *cpu, capacity);
    }
    if let Some(memory) = snapshot.memory.value() {
        append(&mut history.memory_used, memory.used_bytes as f64, capacity);
        append(
            &mut history.swap_used,
            memory.swap_used_bytes as f64,
            capacity,
        );
    }

    match &snapshot.gpus {
        Collection::Available(items) => {
            retain_present(
                &mut history.gpu_utilization,
                items
                    .iter()
                    .filter(|x| x.utilization_percent.is_some())
                    .map(|x| x.id.clone()),
            );
            retain_present(
                &mut history.gpu_memory_used,
                items
                    .iter()
                    .filter(|x| x.memory_used_bytes.is_some())
                    .map(|x| x.id.clone()),
            );
            retain_present(
                &mut history.gpu_temperature,
                items
                    .iter()
                    .filter(|x| x.temperature_celsius.is_some())
                    .map(|x| x.id.clone()),
            );
            append_gpus(history, items, capacity);
        }
        Collection::Degraded(items) => append_gpus(history, items, capacity),
        Collection::Unavailable(_) => {}
    }

    match &snapshot.networks {
        Collection::Available(items) => {
            retain_present(&mut history.networks, items.iter().map(|x| x.id.clone()));
            append_networks(history, items, capacity);
        }
        Collection::Degraded(items) => append_networks(history, items, capacity),
        Collection::Unavailable(_) => {}
    }

    match &snapshot.disks {
        Collection::Available(items) => {
            retain_present(&mut history.disks, items.iter().map(|x| x.id.clone()));
            append_disks(history, items, capacity);
        }
        Collection::Degraded(items) => append_disks(history, items, capacity),
        Collection::Unavailable(_) => {}
    }

    match &snapshot.temperatures {
        Collection::Available(items) => {
            retain_present(
                &mut history.temperatures,
                items.iter().map(|x| x.id.clone()),
            );
            append_temperatures(history, items, capacity);
        }
        Collection::Degraded(items) => append_temperatures(history, items, capacity),
        Collection::Unavailable(_) => {}
    }

    match &snapshot.fans {
        Collection::Available(items) => {
            retain_present(&mut history.fans, items.iter().map(|x| x.id.clone()));
            append_fans(history, items, capacity);
        }
        Collection::Degraded(items) => append_fans(history, items, capacity),
        Collection::Unavailable(_) => {}
    }
}

fn append_gpus(history: &mut MonitorHistory, items: &[super::model::GpuSnapshot], capacity: usize) {
    for item in items {
        if let Some(value) = item.utilization_percent {
            append(
                history.gpu_utilization.entry(item.id.clone()).or_default(),
                value,
                capacity,
            );
        }
        if let Some(value) = item.memory_used_bytes {
            append(
                history.gpu_memory_used.entry(item.id.clone()).or_default(),
                value as f64,
                capacity,
            );
        }
        if let Some(value) = item.temperature_celsius {
            append(
                history.gpu_temperature.entry(item.id.clone()).or_default(),
                value,
                capacity,
            );
        }
    }
}

fn append_networks(
    history: &mut MonitorHistory,
    items: &[super::model::NetworkSnapshot],
    capacity: usize,
) {
    for item in items {
        let entry = history.networks.entry(item.id.clone()).or_default();
        append(&mut entry.down, item.down_bytes_per_sec, capacity);
        append(&mut entry.up, item.up_bytes_per_sec, capacity);
    }
}

fn append_disks(
    history: &mut MonitorHistory,
    items: &[super::model::DiskSnapshot],
    capacity: usize,
) {
    for item in items {
        append(
            history.disks.entry(item.id.clone()).or_default(),
            item.bytes_per_sec,
            capacity,
        );
    }
}

fn append_temperatures(
    history: &mut MonitorHistory,
    items: &[super::model::TemperatureSnapshot],
    capacity: usize,
) {
    for item in items {
        append(
            history.temperatures.entry(item.id.clone()).or_default(),
            item.celsius,
            capacity,
        );
    }
}

fn append_fans(history: &mut MonitorHistory, items: &[super::model::FanSnapshot], capacity: usize) {
    for item in items {
        append(
            history.fans.entry(item.id.clone()).or_default(),
            item.rpm as f64,
            capacity,
        );
    }
}

fn append(values: &mut VecDeque<f64>, value: f64, capacity: usize) {
    if capacity == 0 {
        return;
    }
    if values.len() == capacity {
        values.pop_front();
    }
    values.push_back(value);
}

fn retain_present<K: Clone + Ord, T>(map: &mut BTreeMap<K, T>, ids: impl Iterator<Item = K>) {
    let ids: std::collections::BTreeSet<K> = ids.collect();
    map.retain(|id, _| ids.contains(id));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        CollectionUnavailable, DiskId, DiskSnapshot, GpuId, GpuSnapshot, NetworkId,
        NetworkSnapshot, SystemSnapshot, TemperatureSnapshot,
    };

    #[test]
    fn history_is_bounded() {
        let mut history = MonitorHistory::default();
        for i in 0..5 {
            let snapshot = SystemSnapshot {
                cpu_percent: Collection::available(i as f64),
                ..SystemSnapshot::default()
            };
            push(&mut history, &snapshot, 3);
        }
        assert_eq!(
            history.cpu.into_iter().collect::<Vec<_>>(),
            vec![2.0, 3.0, 4.0]
        );
    }

    #[test]
    fn gpu_histories_are_bounded_and_use_stable_identity() {
        let id = GpuId::from_opaque_key("gpu-a");
        let mut history = MonitorHistory::default();
        for sample in [1_u64, 2, 3, 4] {
            let snapshot = SystemSnapshot {
                gpus: Collection::available(vec![GpuSnapshot {
                    id: id.clone(),
                    name: format!("GPU {sample}"),
                    utilization_percent: Some(sample as f64 * 10.0),
                    memory_used_bytes: Some(sample),
                    memory_total_bytes: Some(8),
                    temperature_celsius: Some(50.0 + sample as f64),
                    power_watts: None,
                    core_clock_mhz: None,
                    fan_percent: None,
                    fan_rpm: None,
                }]),
                ..SystemSnapshot::default()
            };
            push(&mut history, &snapshot, 3);
        }

        assert_eq!(
            history.gpu_utilization[&id]
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![20.0, 30.0, 40.0]
        );
        assert_eq!(
            history.gpu_memory_used[&id]
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![2.0, 3.0, 4.0]
        );
        assert_eq!(
            history.gpu_temperature[&id]
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![52.0, 53.0, 54.0]
        );
    }

    #[test]
    fn temperature_history_uses_stable_identity_not_display_name() {
        let mut history = MonitorHistory::default();
        let snapshot = SystemSnapshot {
            temperatures: Collection::available(vec![TemperatureSnapshot {
                id: "sensor-a".to_owned(),
                name: "GPU".to_owned(),
                celsius: 51.0,
            }]),
            ..SystemSnapshot::default()
        };
        push(&mut history, &snapshot, 3);

        let renamed = SystemSnapshot {
            temperatures: Collection::available(vec![TemperatureSnapshot {
                id: "sensor-a".to_owned(),
                name: "GPU 1".to_owned(),
                celsius: 52.0,
            }]),
            ..SystemSnapshot::default()
        };
        push(&mut history, &renamed, 3);

        assert_eq!(
            history
                .temperatures
                .get("sensor-a")
                .unwrap()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![51.0, 52.0]
        );
        assert!(!history.temperatures.contains_key("GPU"));
    }

    #[test]
    fn network_and_disk_history_follow_identity_across_renames() {
        let network_id = NetworkId::from_opaque_key("network-a");
        let disk_id = DiskId::from_opaque_key("disk-a");
        let mut history = MonitorHistory::default();
        let first = SystemSnapshot {
            networks: Collection::available(vec![NetworkSnapshot {
                id: network_id.clone(),
                name: "eth0".to_owned(),
                down_bytes_per_sec: 10.0,
                up_bytes_per_sec: 20.0,
            }]),
            disks: Collection::available(vec![DiskSnapshot {
                id: disk_id.clone(),
                name: "sda".to_owned(),
                bytes_per_sec: 30.0,
            }]),
            ..SystemSnapshot::default()
        };
        push(&mut history, &first, 3);

        let renamed = SystemSnapshot {
            networks: Collection::available(vec![NetworkSnapshot {
                id: network_id.clone(),
                name: "lan0".to_owned(),
                down_bytes_per_sec: 11.0,
                up_bytes_per_sec: 21.0,
            }]),
            disks: Collection::available(vec![DiskSnapshot {
                id: disk_id.clone(),
                name: "system-disk".to_owned(),
                bytes_per_sec: 31.0,
            }]),
            ..SystemSnapshot::default()
        };
        push(&mut history, &renamed, 3);

        assert_eq!(
            history.networks[&network_id]
                .down
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![10.0, 11.0]
        );
        assert_eq!(
            history.disks[&disk_id].iter().copied().collect::<Vec<_>>(),
            vec![30.0, 31.0]
        );
    }

    #[test]
    fn incomplete_observations_do_not_evict_history() {
        let first_id = NetworkId::from_opaque_key("network-a");
        let second_id = NetworkId::from_opaque_key("network-b");
        let mut history = MonitorHistory::default();
        let complete = SystemSnapshot {
            networks: Collection::available(vec![
                NetworkSnapshot {
                    id: first_id.clone(),
                    name: "eth0".to_owned(),
                    down_bytes_per_sec: 1.0,
                    up_bytes_per_sec: 2.0,
                },
                NetworkSnapshot {
                    id: second_id.clone(),
                    name: "wlan0".to_owned(),
                    down_bytes_per_sec: 3.0,
                    up_bytes_per_sec: 4.0,
                },
            ]),
            ..SystemSnapshot::default()
        };
        push(&mut history, &complete, 3);

        let degraded = SystemSnapshot {
            networks: Collection::degraded(vec![NetworkSnapshot {
                id: first_id.clone(),
                name: "eth0".to_owned(),
                down_bytes_per_sec: 5.0,
                up_bytes_per_sec: 6.0,
            }]),
            ..SystemSnapshot::default()
        };
        push(&mut history, &degraded, 3);
        assert!(history.networks.contains_key(&second_id));

        let unavailable = SystemSnapshot {
            networks: Collection::unavailable(CollectionUnavailable::PermissionDenied),
            ..SystemSnapshot::default()
        };
        push(&mut history, &unavailable, 3);
        assert!(history.networks.contains_key(&first_id));
        assert!(history.networks.contains_key(&second_id));

        let empty = SystemSnapshot {
            networks: Collection::available(Vec::new()),
            ..SystemSnapshot::default()
        };
        push(&mut history, &empty, 3);
        assert!(history.networks.is_empty());
    }
}
