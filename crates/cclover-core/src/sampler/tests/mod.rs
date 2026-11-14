use crate::model::*;

mod process;
mod ranking;
mod system;

pub(super) fn process_id(pid: u32, instance: u64) -> ProcessInstanceId {
    ProcessInstanceId { pid, instance }
}

pub(super) fn network_id(key: &str) -> NetworkId {
    NetworkId::from_opaque_key(key)
}

pub(super) fn disk_id(key: &str) -> DiskId {
    DiskId::from_opaque_key(key)
}

/// Row counts used by the shared dashboard card regions.
pub(super) const RANK_ROWS: usize = 8;
pub(super) const IO_RANK_ROWS: usize = 3;

pub(super) fn top_cpu(out: &SystemSnapshot) -> Collection<Vec<ProcessCpuRank>> {
    out.processes.cpu_ranking(RANK_ROWS)
}

pub(super) fn top_memory(out: &SystemSnapshot) -> Collection<Vec<ProcessMemoryRank>> {
    out.processes.memory_ranking(RANK_ROWS)
}

pub(super) fn disk_io(out: &SystemSnapshot) -> Collection<Vec<ProcessDiskIo>> {
    out.processes.disk_io_ranking(IO_RANK_ROWS)
}

pub(super) fn network_io(out: &SystemSnapshot) -> Collection<Vec<ProcessNetworkIo>> {
    out.processes.network_io_ranking(IO_RANK_ROWS)
}

pub(super) fn disk_metadata(system_label: &str) -> DiskMetadata {
    DiskMetadata {
        system_label: system_label.to_owned(),
        associated_labels: Vec::new(),
    }
}
