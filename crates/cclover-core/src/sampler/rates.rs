use std::collections::HashMap;

use super::status::map_status;
use crate::model::*;

pub(super) fn sample_interval_seconds(
    previous: Option<&RawSnapshot>,
    current: &RawSnapshot,
) -> f64 {
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

pub(super) fn derive_networks(
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

pub(super) fn derive_disks(
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
                metadata: item.metadata.clone(),
                bytes_per_sec: rate,
            }
        })
        .collect();
    map_status(&current.disks, values)
}
