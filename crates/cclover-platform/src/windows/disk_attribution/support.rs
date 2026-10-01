use super::*;

pub(super) fn unavailable_from_etw_io(
    error: &io::Error,
) -> cclover_core::model::CollectionUnavailable {
    if error.raw_os_error() == Some(5) {
        cclover_core::model::CollectionUnavailable::PermissionDenied
    } else {
        unavailable_from_io(error)
    }
}

pub(super) fn collection_for_status(
    status: CollectionStatus,
    rows: Vec<ProcessDiskIoCounter>,
) -> Collection<Vec<ProcessDiskIoCounter>> {
    match status {
        CollectionStatus::Available => Collection::available(rows),
        CollectionStatus::Degraded => Collection::degraded(rows),
        CollectionStatus::Unavailable(reason) => Collection::unavailable(reason),
    }
}

pub(super) fn complete_process_map(
    processes: &Collection<Vec<ProcessCounter>>,
) -> Option<HashMap<u32, ProcessInstanceId>> {
    match processes {
        Collection::Available(processes) => Some(
            processes
                .iter()
                .map(|process| (process.process.pid, process.process))
                .collect(),
        ),
        Collection::Degraded(_) | Collection::Unavailable(_) => None,
    }
}

pub(super) fn stable_process_identity(
    pid: u32,
    previous: &HashMap<u32, ProcessInstanceId>,
    current: &HashMap<u32, ProcessInstanceId>,
) -> Option<ProcessInstanceId> {
    let previous = previous.get(&pid)?;
    let current = current.get(&pid)?;
    (previous == current).then_some(*current)
}

pub(super) fn volume_path_matches(path: &str, volume: &str) -> bool {
    path == volume
        || path
            .strip_prefix(volume)
            .is_some_and(|rest| rest.starts_with('\\'))
}

pub(super) fn accumulate_interval(
    totals: &mut BTreeMap<AttributionKey, Total>,
    interval: HashMap<IntervalKey, u64>,
    previous_processes: &HashMap<u32, ProcessInstanceId>,
    current_processes: &HashMap<u32, ProcessInstanceId>,
    disks: &HashMap<u32, DiskIdentity>,
    unresolved_disks: &mut usize,
) {
    for (key, bytes) in interval {
        let Some(process) = stable_process_identity(key.pid, previous_processes, current_processes)
        else {
            continue;
        };
        let Some(disk) = disks.get(&key.disk_number) else {
            *unresolved_disks = unresolved_disks.saturating_add(1);
            continue;
        };
        let total = totals
            .entry(AttributionKey {
                process,
                disk_id: disk.id.clone(),
            })
            .or_insert_with(|| Total {
                device: disk.device.clone(),
                read_bytes: 0,
                write_bytes: 0,
            });
        total.device.clone_from(&disk.device);
        match key.direction {
            DirectionKey::Read => total.read_bytes = total.read_bytes.saturating_add(bytes),
            DirectionKey::Write => total.write_bytes = total.write_bytes.saturating_add(bytes),
        }
    }
}
