use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use crate::model::*;
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
