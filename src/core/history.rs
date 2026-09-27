use std::collections::{BTreeMap, VecDeque};

use super::model::{MonitorHistory, SystemSnapshot};

pub fn push(history: &mut MonitorHistory, snapshot: &SystemSnapshot, capacity: usize) {
    if let Some(cpu) = snapshot.cpu_percent {
        append(&mut history.cpu, cpu, capacity);
    }
    if let Some(memory) = &snapshot.memory {
        append(&mut history.memory_used, memory.used_bytes as f64, capacity);
        append(
            &mut history.swap_used,
            memory.swap_used_bytes as f64,
            capacity,
        );
    }

    retain_present(
        &mut history.networks,
        snapshot.networks.iter().map(|x| x.id.clone()),
    );
    for item in &snapshot.networks {
        let entry = history.networks.entry(item.id.clone()).or_default();
        append(&mut entry.down, item.down_bytes_per_sec, capacity);
        append(&mut entry.up, item.up_bytes_per_sec, capacity);
    }

    retain_present(
        &mut history.disks,
        snapshot.disks.iter().map(|x| x.id.clone()),
    );
    for item in &snapshot.disks {
        append(
            history.disks.entry(item.id.clone()).or_default(),
            item.bytes_per_sec,
            capacity,
        );
    }

    retain_present(
        &mut history.temperatures,
        snapshot.temperatures.iter().map(|x| x.id.clone()),
    );
    for item in &snapshot.temperatures {
        append(
            history.temperatures.entry(item.id.clone()).or_default(),
            item.celsius,
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
    use crate::core::model::{
        DiskId, DiskSnapshot, NetworkId, NetworkSnapshot, SystemSnapshot, TemperatureSnapshot,
    };

    #[test]
    fn history_is_bounded() {
        let mut history = MonitorHistory::default();
        for i in 0..5 {
            let snapshot = SystemSnapshot {
                cpu_percent: Some(i as f64),
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
    fn temperature_history_uses_stable_identity_not_display_name() {
        let mut history = MonitorHistory::default();
        let snapshot = SystemSnapshot {
            temperatures: vec![TemperatureSnapshot {
                id: "sensor-a".to_owned(),
                name: "GPU".to_owned(),
                celsius: 51.0,
            }],
            ..SystemSnapshot::default()
        };
        push(&mut history, &snapshot, 3);

        let renamed = SystemSnapshot {
            temperatures: vec![TemperatureSnapshot {
                id: "sensor-a".to_owned(),
                name: "GPU 1".to_owned(),
                celsius: 52.0,
            }],
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
            networks: vec![NetworkSnapshot {
                id: network_id.clone(),
                name: "eth0".to_owned(),
                down_bytes_per_sec: 10.0,
                up_bytes_per_sec: 20.0,
            }],
            disks: vec![DiskSnapshot {
                id: disk_id.clone(),
                name: "sda".to_owned(),
                bytes_per_sec: 30.0,
            }],
            ..SystemSnapshot::default()
        };
        push(&mut history, &first, 3);

        let renamed = SystemSnapshot {
            networks: vec![NetworkSnapshot {
                id: network_id.clone(),
                name: "lan0".to_owned(),
                down_bytes_per_sec: 11.0,
                up_bytes_per_sec: 21.0,
            }],
            disks: vec![DiskSnapshot {
                id: disk_id.clone(),
                name: "system-disk".to_owned(),
                bytes_per_sec: 31.0,
            }],
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
}
