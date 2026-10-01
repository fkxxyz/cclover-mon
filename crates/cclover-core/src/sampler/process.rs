use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use crate::model::*;

pub(super) const TOP_N: usize = 8;
pub(super) const PROCESS_IO_TOP_N: usize = 3;

pub(super) fn derive_process_domain(
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

pub(super) fn derive_process_disk_io_rates(
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

pub(super) fn derive_process_network_io_rates(
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

pub(super) fn top_cpu(processes: &ProcessDomainSnapshot) -> Vec<ProcessCpuUsage> {
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

pub(super) fn top_memory(processes: &ProcessDomainSnapshot) -> Vec<ProcessMemoryUsage> {
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

pub(super) fn project_process_disk_io(processes: &ProcessDomainSnapshot) -> Vec<ProcessDiskIo> {
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

pub(super) fn project_process_network_io(
    processes: &ProcessDomainSnapshot,
) -> Vec<ProcessNetworkIo> {
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
