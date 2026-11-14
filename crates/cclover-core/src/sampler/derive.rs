use crate::model::*;

use super::cpu::derive_cpu;
use super::process::derive_process_domain;
use super::rates::{derive_disks, derive_networks, sample_interval_seconds};

pub(super) fn derive(previous: Option<&RawSnapshot>, current: &RawSnapshot) -> SystemSnapshot {
    let current_processes = current
        .processes
        .value()
        .map(Vec::as_slice)
        .unwrap_or_default();
    let (cpu_percent, process_cpu) = derive_cpu(previous, current, current_processes);
    let dt = sample_interval_seconds(previous, current);
    let processes = derive_process_domain(previous, current, current_processes, &process_cpu, dt);

    SystemSnapshot {
        cpu_percent,
        memory: current.memory.clone(),
        processes,
        networks: derive_networks(previous, current, dt),
        disks: derive_disks(previous, current, dt),
        temperatures: current.temperatures.clone(),
        fans: current.fans.clone(),
        gpus: current.gpus.clone(),
    }
}
