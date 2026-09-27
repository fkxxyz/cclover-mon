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
        snapshot.networks.iter().map(|x| x.name.as_str()),
    );
    for item in &snapshot.networks {
        let entry = history.networks.entry(item.name.clone()).or_default();
        append(&mut entry.down, item.down_bytes_per_sec, capacity);
        append(&mut entry.up, item.up_bytes_per_sec, capacity);
    }

    retain_present(
        &mut history.disks,
        snapshot.disks.iter().map(|x| x.name.as_str()),
    );
    for item in &snapshot.disks {
        append(
            history.disks.entry(item.name.clone()).or_default(),
            item.bytes_per_sec,
            capacity,
        );
    }

    retain_present(
        &mut history.temperatures,
        snapshot.temperatures.iter().map(|x| x.id.as_str()),
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

fn retain_present<T>(map: &mut BTreeMap<String, T>, names: impl Iterator<Item = impl AsRef<str>>) {
    let names: std::collections::BTreeSet<String> = names.map(|x| x.as_ref().to_owned()).collect();
    map.retain(|name, _| names.contains(name));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::model::{SystemSnapshot, TemperatureSnapshot};

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
}
