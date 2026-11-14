//! Ranked process views over the shared process domain.
//!
//! The domain in [`ProcessDomainSnapshot`] is the only process fact source. Ranking is a pure
//! query over it: how many rows a caller needs is that caller's policy, so it is a parameter here
//! rather than a stored snapshot field. Consumers that render or serialize process rows call these
//! with their own limits and never re-derive rates, joins, or device attribution.
//!
//! Every ranking is deterministic: rows are ordered by descending value, and equal values are
//! ordered by ascending [`ProcessInstanceId`]. A process without an observed name is still ranked;
//! its [`ProcessRef::name`] is simply `None`.

use std::collections::BTreeMap;

use crate::model::*;

impl ProcessDomainSnapshot {
    pub fn cpu_ranking(&self, limit: usize) -> Collection<Vec<ProcessCpuRank>> {
        rank(self.cpu_status, || {
            let mut rows: Vec<(&ProcessSnapshot, f64)> = self
                .by_id
                .values()
                .filter_map(|process| Some((process, process.cpu_percent?)))
                .collect();
            keep_top_n_by(&mut rows, limit, |a, b| {
                b.1.total_cmp(&a.1).then_with(|| a.0.id.cmp(&b.0.id))
            });
            rows.into_iter()
                .map(|(process, percent)| ProcessCpuRank {
                    process: ProcessRef::from(process),
                    percent,
                })
                .collect()
        })
    }

    pub fn memory_ranking(&self, limit: usize) -> Collection<Vec<ProcessMemoryRank>> {
        rank(self.memory_status, || {
            let mut rows: Vec<(&ProcessSnapshot, u64)> = self
                .by_id
                .values()
                .filter_map(|process| Some((process, process.memory_bytes?)))
                .collect();
            keep_top_n_by(&mut rows, limit, |a, b| {
                b.1.cmp(&a.1).then_with(|| a.0.id.cmp(&b.0.id))
            });
            rows.into_iter()
                .map(|(process, bytes)| ProcessMemoryRank {
                    process: ProcessRef::from(process),
                    bytes,
                })
                .collect()
        })
    }

    /// Ranks per-disk I/O rows, keeping the highest `per_device_limit` rows for each disk.
    pub fn disk_io_ranking(&self, per_device_limit: usize) -> Collection<Vec<ProcessDiskIo>> {
        rank(self.disk_io_status, || {
            let mut by_disk: BTreeMap<DiskId, Vec<ProcessDiskIo>> = BTreeMap::new();
            for process in self.by_id.values() {
                for io in &process.disk_io {
                    by_disk
                        .entry(io.disk_id.clone())
                        .or_default()
                        .push(ProcessDiskIo {
                            process: ProcessRef::from(process),
                            disk_id: io.disk_id.clone(),
                            device: io.device.clone(),
                            read_bytes_per_sec: io.read_bytes_per_sec,
                            write_bytes_per_sec: io.write_bytes_per_sec,
                        });
                }
            }
            rank_per_device(by_disk, per_device_limit, |row| {
                row.read_bytes_per_sec + row.write_bytes_per_sec
            })
        })
    }

    /// Ranks per-interface network I/O rows, keeping the highest `per_device_limit` rows per
    /// interface.
    pub fn network_io_ranking(&self, per_device_limit: usize) -> Collection<Vec<ProcessNetworkIo>> {
        rank(self.network_io_status, || {
            let mut by_network: BTreeMap<NetworkId, Vec<ProcessNetworkIo>> = BTreeMap::new();
            for process in self.by_id.values() {
                for io in &process.network_io {
                    by_network
                        .entry(io.network_id.clone())
                        .or_default()
                        .push(ProcessNetworkIo {
                            process: ProcessRef::from(process),
                            network_id: io.network_id.clone(),
                            interface: io.interface.clone(),
                            rx_bytes_per_sec: io.rx_bytes_per_sec,
                            tx_bytes_per_sec: io.tx_bytes_per_sec,
                        });
                }
            }
            rank_per_device(by_network, per_device_limit, |row| {
                row.rx_bytes_per_sec + row.tx_bytes_per_sec
            })
        })
    }
}

/// Ranks only when the observation is usable; an unavailable metric is not ranked at all.
fn rank<T>(status: CollectionStatus, rows: impl FnOnce() -> Vec<T>) -> Collection<Vec<T>> {
    match status {
        CollectionStatus::Unavailable(reason) => Collection::Unavailable(reason),
        status => status.wrap(rows()),
    }
}

fn rank_per_device<K: Ord, T>(
    mut by_device: BTreeMap<K, Vec<T>>,
    per_device_limit: usize,
    total: impl Fn(&T) -> f64,
) -> Vec<T> {
    for rows in by_device.values_mut() {
        keep_top_n_by(rows, per_device_limit, |a, b| total(b).total_cmp(&total(a)));
    }
    by_device.into_values().flatten().collect()
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
    use std::sync::Arc;

    use super::*;

    fn process(id: ProcessInstanceId, name: Option<&str>) -> ProcessSnapshot {
        ProcessSnapshot {
            id,
            name: name.map(Arc::from),
            cpu_percent: None,
            memory_bytes: None,
            disk_io: Vec::new(),
            network_io: Vec::new(),
        }
    }

    fn id(pid: u32, instance: u64) -> ProcessInstanceId {
        ProcessInstanceId { pid, instance }
    }

    fn domain(processes: Vec<ProcessSnapshot>) -> ProcessDomainSnapshot {
        ProcessDomainSnapshot {
            by_id: Arc::new(
                processes
                    .into_iter()
                    .map(|process| (process.id, process))
                    .collect(),
            ),
            metadata_status: CollectionStatus::Available,
            cpu_status: CollectionStatus::Available,
            memory_status: CollectionStatus::Available,
            disk_io_status: CollectionStatus::Available,
            network_io_status: CollectionStatus::Available,
        }
    }

    #[test]
    fn cpu_ranking_orders_by_value_then_identity_and_keeps_the_requested_rows() {
        let processes = (0..12)
            .map(|index| ProcessSnapshot {
                cpu_percent: Some(f64::from(index)),
                ..process(id(index, 1), Some("worker"))
            })
            .collect();
        let ranks = domain(processes).cpu_ranking(3);
        let ranks = ranks.value().unwrap();

        assert_eq!(ranks.len(), 3);
        assert_eq!(ranks[0].process.id.pid, 11);
        assert_eq!(ranks[2].process.id.pid, 9);
        assert_eq!(ranks[0].process.name.as_deref(), Some("worker"));
    }

    #[test]
    fn equal_values_are_ordered_by_process_identity() {
        let processes = vec![
            ProcessSnapshot {
                cpu_percent: Some(5.0),
                ..process(id(30, 1), Some("a"))
            },
            ProcessSnapshot {
                cpu_percent: Some(5.0),
                ..process(id(10, 1), Some("b"))
            },
        ];

        let ranks = domain(processes).cpu_ranking(2);
        let pids: Vec<_> = ranks
            .value()
            .unwrap()
            .iter()
            .map(|rank| rank.process.id.pid)
            .collect();

        assert_eq!(pids, [10, 30]);
    }

    #[test]
    fn unnamed_processes_keep_their_rank_instead_of_being_dropped() {
        let processes = vec![
            ProcessSnapshot {
                memory_bytes: Some(4096),
                ..process(id(7, 3), None)
            },
            ProcessSnapshot {
                memory_bytes: Some(1024),
                ..process(id(8, 4), Some("named"))
            },
        ];

        let ranks = domain(processes).memory_ranking(2);
        let ranks = ranks.value().unwrap();

        assert_eq!(ranks.len(), 2);
        assert_eq!(ranks[0].process.id.pid, 7);
        assert_eq!(ranks[0].process.name, None);
    }

    #[test]
    fn rankings_carry_the_observation_status_of_their_metric() {
        let mut domain = domain(Vec::new());
        domain.cpu_status = CollectionStatus::Unavailable(CollectionUnavailable::PermissionDenied);

        let ranks = domain.cpu_ranking(8);

        assert_eq!(
            ranks.value(),
            None,
            "an unavailable metric must not fall back to an empty but available ranking"
        );
        assert_eq!(
            ranks,
            Collection::Unavailable(CollectionUnavailable::PermissionDenied)
        );
    }

    #[test]
    fn a_degraded_metric_still_yields_its_rows() {
        let processes = vec![ProcessSnapshot {
            cpu_percent: Some(4.0),
            ..process(id(5, 1), Some("partial"))
        }];
        let mut domain = domain(processes);
        domain.cpu_status = CollectionStatus::Degraded;

        let ranks = domain.cpu_ranking(8);

        assert_eq!(ranks.status(), CollectionStatus::Degraded);
        let ranks = ranks.value().expect("a degraded metric is usable data");
        assert_eq!(ranks.len(), 1);
        assert_eq!(ranks[0].process.id.pid, 5);
    }

    #[test]
    fn io_rankings_keep_the_highest_rows_per_device() {
        let disk_id = DiskId::from_opaque_key("disk-a");
        let other_disk_id = DiskId::from_opaque_key("disk-b");
        let processes = vec![
            ProcessSnapshot {
                disk_io: vec![
                    ProcessDiskIoSnapshot {
                        disk_id: disk_id.clone(),
                        device: "sda".into(),
                        read_bytes_per_sec: 10.0,
                        write_bytes_per_sec: 0.0,
                    },
                    ProcessDiskIoSnapshot {
                        disk_id: other_disk_id.clone(),
                        device: "sdb".into(),
                        read_bytes_per_sec: 1.0,
                        write_bytes_per_sec: 0.0,
                    },
                ],
                ..process(id(1, 1), Some("first"))
            },
            ProcessSnapshot {
                disk_io: vec![ProcessDiskIoSnapshot {
                    disk_id: disk_id.clone(),
                    device: "sda".into(),
                    read_bytes_per_sec: 20.0,
                    write_bytes_per_sec: 0.0,
                }],
                ..process(id(2, 1), Some("second"))
            },
        ];

        let rows = domain(processes).disk_io_ranking(1);
        let rows = rows.value().unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].disk_id, disk_id);
        assert_eq!(rows[0].process.id.pid, 2);
        assert_eq!(rows[1].disk_id, other_disk_id);
        assert_eq!(rows[1].process.id.pid, 1);
    }
}
