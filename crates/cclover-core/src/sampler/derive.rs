use crate::model::*;

use super::cpu::derive_cpu;
use super::process::{
    derive_process_domain, project_process_disk_io, project_process_network_io, top_cpu, top_memory,
};
use super::rates::{derive_disks, derive_networks, sample_interval_seconds};
use super::status::collection_from_status;

pub(super) fn derive(previous: Option<&RawSnapshot>, current: &RawSnapshot) -> SystemSnapshot {
    let current_processes = current
        .processes
        .value()
        .map(Vec::as_slice)
        .unwrap_or_default();
    let (cpu_percent, process_cpu) = derive_cpu(previous, current, current_processes);
    let dt = sample_interval_seconds(previous, current);
    let processes = derive_process_domain(previous, current, current_processes, &process_cpu, dt);
    let top_cpu = collection_from_status(processes.cpu_status, top_cpu(&processes));
    let top_memory = collection_from_status(processes.memory_status, top_memory(&processes));
    let process_disk_io = collection_from_status(
        processes.disk_io_status,
        project_process_disk_io(&processes),
    );
    let process_network_io = collection_from_status(
        processes.network_io_status,
        project_process_network_io(&processes),
    );

    SystemSnapshot {
        cpu_percent,
        memory: current.memory.clone(),
        processes,
        top_cpu,
        top_memory,
        networks: derive_networks(previous, current, dt),
        disks: derive_disks(previous, current, dt),
        process_disk_io,
        process_network_io,
        temperatures: current.temperatures.clone(),
        fans: current.fans.clone(),
        gpus: current.gpus.clone(),
    }
}
