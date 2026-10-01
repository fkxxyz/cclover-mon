use crate::model::*;

mod process;
mod ranking;
mod system;

pub(super) fn process_id(pid: u32, birth_marker: u64) -> ProcessInstanceId {
    ProcessInstanceId { pid, birth_marker }
}

pub(super) fn network_id(key: &str) -> NetworkId {
    NetworkId::from_opaque_key(key)
}

pub(super) fn disk_id(key: &str) -> DiskId {
    DiskId::from_opaque_key(key)
}

pub(super) fn disk_metadata(system_label: &str) -> DiskMetadata {
    DiskMetadata {
        system_label: system_label.to_owned(),
        associated_labels: Vec::new(),
    }
}
