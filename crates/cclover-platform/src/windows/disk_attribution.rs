use std::collections::{BTreeMap, HashMap, HashSet};
use std::io;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cclover_core::model::{
    Collection, CollectionStatus, DiskId, ProcessCounter, ProcessDiskIoCounter, ProcessInstanceId,
};

use super::diagnostics::{report_issue, unavailable_from_io};
use super::disk::{self, DiskIdentity};
use super::etw::{self, Direction, Event, EventSink};
use super::native;

const RETRY_INTERVAL: Duration = Duration::from_secs(30);
const TOPOLOGY_REFRESH_INTERVAL: Duration = Duration::from_secs(30);
const MAX_THREADS: usize = 262_144;
const MAX_FILES: usize = 262_144;
const MAX_PENDING: usize = 131_072;
const MAX_INTERVAL_KEYS: usize = 131_072;
const IRP_PAGING_IO: u32 = 0x0000_0002;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct AttributionKey {
    process: ProcessInstanceId,
    disk_id: DiskId,
}

#[derive(Clone, Debug)]
struct Total {
    device: String,
    read_bytes: u64,
    write_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct IntervalKey {
    pid: u32,
    disk_number: u32,
    direction: DirectionKey,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum DirectionKey {
    Read,
    Write,
}

impl From<Direction> for DirectionKey {
    fn from(value: Direction) -> Self {
        match value {
            Direction::Read => Self::Read,
            Direction::Write => Self::Write,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FileTarget {
    SingleDisk(u32),
    Ambiguous,
}

#[derive(Clone, Copy, Debug)]
struct PendingIo {
    pid: u32,
    target: FileTarget,
    direction: DirectionKey,
    requested_bytes: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct VolumeRoute {
    nt_path_lower: Arc<str>,
    target: FileTarget,
}

#[derive(Default)]
struct IntervalStats {
    ambiguous_volume_ios: usize,
    unmapped_file_ios: usize,
    unresolved_local_files: usize,
    invalid_completions: usize,
    state_overflowed: bool,
}

#[derive(Default)]
struct Interval {
    bytes: HashMap<IntervalKey, u64>,
    stats: IntervalStats,
}

struct SinkState {
    topology: Vec<VolumeRoute>,
    threads: HashMap<u32, u32>,
    files: HashMap<u64, Arc<str>>,
    pending: HashMap<u64, PendingIo>,
    interval: Interval,
    file_map_overflowed: bool,
}

impl SinkState {
    fn new() -> Self {
        Self {
            topology: Vec::new(),
            threads: HashMap::new(),
            files: HashMap::new(),
            pending: HashMap::new(),
            interval: Interval::default(),
            file_map_overflowed: false,
        }
    }

    fn update_topology(&mut self, topology: Vec<VolumeRoute>) {
        if self.topology == topology {
            return;
        }
        self.topology = topology;
    }

    fn reset_epoch(&mut self) {
        self.threads.clear();
        self.files.clear();
        self.pending.clear();
        self.interval = Interval::default();
        self.file_map_overflowed = false;
    }

    fn reset_session_state_preserving_files(&mut self) {
        self.threads.clear();
        self.pending.clear();
        self.interval = Interval::default();
    }

    fn bounded_insert<K: std::hash::Hash + Eq, V>(
        map: &mut HashMap<K, V>,
        key: K,
        value: V,
        limit: usize,
    ) -> bool {
        if map.len() >= limit && !map.contains_key(&key) {
            return false;
        }
        map.insert(key, value);
        true
    }

    fn volume_for_path(&self, path: &str) -> Option<Arc<str>> {
        let lower = path.to_ascii_lowercase();
        self.topology
            .iter()
            .filter(|route| volume_path_matches(&lower, &route.nt_path_lower))
            .max_by_key(|route| route.nt_path_lower.len())
            .map(|route| route.nt_path_lower.clone())
    }

    fn target_for_volume(&self, volume: &str) -> Option<FileTarget> {
        self.topology
            .iter()
            .find(|route| route.nt_path_lower.as_ref() == volume)
            .map(|route| route.target)
    }

    fn remember_file(&mut self, key: u64, path: &str) {
        let Some(volume) = self.volume_for_path(path) else {
            if path
                .to_ascii_lowercase()
                .starts_with(r"\device\harddiskvolume")
            {
                self.interval.stats.unresolved_local_files =
                    self.interval.stats.unresolved_local_files.saturating_add(1);
            }
            return;
        };
        if !Self::bounded_insert(&mut self.files, key, volume, MAX_FILES) {
            self.interval.stats.state_overflowed = true;
            self.file_map_overflowed = true;
        }
    }

    fn start_io(
        &mut self,
        irp: u64,
        tid: u32,
        file_object: u64,
        file_key: u64,
        direction: Direction,
        requested_bytes: u32,
        irp_flags: u32,
    ) {
        if irp_flags & IRP_PAGING_IO != 0 {
            return;
        }
        let Some(&pid) = self.threads.get(&tid) else {
            return;
        };
        let volume = self
            .files
            .get(&file_key)
            .or_else(|| self.files.get(&file_object))
            .cloned();
        let Some(volume) = volume else {
            self.interval.stats.unmapped_file_ios =
                self.interval.stats.unmapped_file_ios.saturating_add(1);
            return;
        };
        let Some(target) = self.target_for_volume(&volume) else {
            self.interval.stats.unresolved_local_files =
                self.interval.stats.unresolved_local_files.saturating_add(1);
            return;
        };
        let pending = PendingIo {
            pid,
            target,
            direction: direction.into(),
            requested_bytes,
        };
        if !Self::bounded_insert(&mut self.pending, irp, pending, MAX_PENDING) {
            self.interval.stats.state_overflowed = true;
        }
    }

    fn finish_io(&mut self, irp: u64, extra_info: u64, status: u32) {
        let Some(pending) = self.pending.remove(&irp) else {
            return;
        };
        if (status as i32) < 0 || extra_info == 0 {
            return;
        }
        if extra_info > u64::from(pending.requested_bytes) {
            self.interval.stats.invalid_completions =
                self.interval.stats.invalid_completions.saturating_add(1);
            return;
        }
        let FileTarget::SingleDisk(disk_number) = pending.target else {
            self.interval.stats.ambiguous_volume_ios =
                self.interval.stats.ambiguous_volume_ios.saturating_add(1);
            return;
        };
        let key = IntervalKey {
            pid: pending.pid,
            disk_number,
            direction: pending.direction,
        };
        if self.interval.bytes.len() >= MAX_INTERVAL_KEYS && !self.interval.bytes.contains_key(&key)
        {
            self.interval.stats.state_overflowed = true;
            return;
        }
        let bytes = self.interval.bytes.entry(key).or_default();
        *bytes = bytes.saturating_add(extra_info);
    }
}

struct Sink {
    state: Mutex<SinkState>,
}

impl Sink {
    fn new() -> Self {
        Self {
            state: Mutex::new(SinkState::new()),
        }
    }

    fn update_topology(&self, topology: Vec<VolumeRoute>) {
        self.state
            .lock()
            .expect("ETW disk attribution state poisoned")
            .update_topology(topology);
    }

    fn take_interval(&self) -> Interval {
        let mut state = self
            .state
            .lock()
            .expect("ETW disk attribution state poisoned");
        std::mem::take(&mut state.interval)
    }

    fn reset_epoch(&self) {
        self.state
            .lock()
            .expect("ETW disk attribution state poisoned")
            .reset_epoch();
    }

    fn reset_session_state_preserving_files(&self) {
        self.state
            .lock()
            .expect("ETW disk attribution state poisoned")
            .reset_session_state_preserving_files();
    }

    fn file_map_incomplete(&self) -> bool {
        self.state
            .lock()
            .expect("ETW disk attribution state poisoned")
            .file_map_overflowed
    }
}

impl EventSink for Sink {
    fn on_event(&self, event: Event) {
        let mut state = self
            .state
            .lock()
            .expect("ETW disk attribution state poisoned");
        match event {
            Event::ThreadStart { pid, tid } => {
                if !SinkState::bounded_insert(&mut state.threads, tid, pid, MAX_THREADS) {
                    state.interval.stats.state_overflowed = true;
                }
            }
            Event::ThreadEnd { tid } => {
                state.threads.remove(&tid);
            }
            Event::FileName { key, path } => state.remember_file(key, &path),
            Event::FileCreate { file_object, path } => state.remember_file(file_object, &path),
            Event::FileObjectEnd { file_object } => {
                state.files.remove(&file_object);
            }
            Event::FileKeyEnd { key } => {
                state.files.remove(&key);
            }
            Event::IoStart {
                irp,
                tid,
                file_object,
                file_key,
                direction,
                requested_bytes,
                irp_flags,
            } => state.start_io(
                irp,
                tid,
                file_object,
                file_key,
                direction,
                requested_bytes,
                irp_flags,
            ),
            Event::IoEnd {
                irp,
                extra_info,
                status,
            } => state.finish_io(irp, extra_info, status),
        }
    }
}

pub(super) struct Collector {
    sink: Arc<Sink>,
    session: Option<etw::Session>,
    retry_after: Option<(Instant, cclover_core::model::CollectionUnavailable)>,
    interval_start_processes: Option<HashMap<u32, ProcessInstanceId>>,
    totals: BTreeMap<AttributionKey, Total>,
    last_health: etw::Health,
    topology_refreshed_at: Option<Instant>,
    topology_failures: usize,
    topology_ready: bool,
    seeded: bool,
}

impl Collector {
    pub(super) fn new() -> Self {
        Self {
            sink: Arc::new(Sink::new()),
            session: None,
            retry_after: None,
            interval_start_processes: None,
            totals: BTreeMap::new(),
            last_health: etw::Health::default(),
            topology_refreshed_at: None,
            topology_failures: 0,
            topology_ready: false,
            seeded: false,
        }
    }

    pub(super) fn collect(
        &mut self,
        processes: &Collection<Vec<ProcessCounter>>,
        disks: &disk::Batch,
        mut notes: Option<&mut Vec<String>>,
    ) -> Collection<Vec<ProcessDiskIoCounter>> {
        if let CollectionStatus::Unavailable(reason) = disks.counters.status() {
            return Collection::unavailable(reason);
        }
        let current_processes = complete_process_map(processes);

        if let Err(error) = self.refresh_topology_if_needed() {
            report_issue(&mut notes, || {
                format!("ETW disk volume topology refresh failed: {error}")
            });
            return Collection::unavailable(unavailable_from_io(&error));
        }

        if self.session.is_none() {
            if let Some((retry_after, reason)) = self.retry_after {
                if Instant::now() < retry_after {
                    return Collection::unavailable(reason);
                }
                self.retry_after = None;
            }
            if !self.seeded {
                match etw::Session::start(self.sink.clone()) {
                    Ok(seed_session) => {
                        // FileRundown is emitted when the SystemTraceProvider session stops. The
                        // synchronous drop waits for the consumer, seeding file-key mappings for
                        // files that were already open before cclover-mon started.
                        drop(seed_session);
                        self.sink.reset_session_state_preserving_files();
                        self.seeded = true;
                    }
                    Err(error) => {
                        let reason = unavailable_from_etw_io(&error);
                        self.retry_after = Some((Instant::now() + RETRY_INTERVAL, reason));
                        report_issue(&mut notes, || {
                            format!("ETW disk attribution seed session failed: {error}")
                        });
                        return Collection::unavailable(reason);
                    }
                }
            }
            match etw::Session::start(self.sink.clone()) {
                Ok(session) => {
                    let health = match session.health() {
                        Ok(health) => health,
                        Err(error) => {
                            let reason = unavailable_from_etw_io(&error);
                            report_issue(&mut notes, || {
                                format!(
                                    "ETW disk attribution health query failed after start: {error}"
                                )
                            });
                            drop(session);
                            self.sink.reset_session_state_preserving_files();
                            self.retry_after = Some((Instant::now() + RETRY_INTERVAL, reason));
                            self.interval_start_processes = None;
                            return Collection::unavailable(reason);
                        }
                    };
                    self.last_health = health;
                    self.session = Some(session);
                    self.interval_start_processes = current_processes;
                    let rows = self.rows();
                    return match processes.status() {
                        CollectionStatus::Unavailable(reason) => Collection::unavailable(reason),
                        CollectionStatus::Degraded => Collection::degraded(rows),
                        CollectionStatus::Available
                            if self.topology_failures > 0
                                || self.sink.file_map_incomplete()
                                || disks.counters.status() == CollectionStatus::Degraded =>
                        {
                            Collection::degraded(rows)
                        }
                        CollectionStatus::Available => Collection::available(rows),
                    };
                }
                Err(error) => {
                    let reason = unavailable_from_etw_io(&error);
                    self.seeded = false;
                    self.sink.reset_epoch();
                    self.retry_after = Some((Instant::now() + RETRY_INTERVAL, reason));
                    report_issue(&mut notes, || {
                        format!("ETW disk attribution session start failed: {error}")
                    });
                    return Collection::unavailable(reason);
                }
            }
        }

        let health = match self
            .session
            .as_ref()
            .expect("ETW session initialized")
            .health()
        {
            Ok(health) => health,
            Err(error) => {
                return self.fail_session(error, &mut notes);
            }
        };
        if health.consumer_failed {
            return self.fail_session(
                io::Error::other("ETW disk attribution consumer failed"),
                &mut notes,
            );
        }
        if health.events_lost > self.last_health.events_lost
            || health.decode_errors > self.last_health.decode_errors
        {
            report_issue(&mut notes, || {
                format!(
                    "ETW disk attribution epoch reset after loss/decode failure: events_lost {}->{}, decode_errors {}->{}",
                    self.last_health.events_lost,
                    health.events_lost,
                    self.last_health.decode_errors,
                    health.decode_errors
                )
            });
            // A controlled stop emits FileRundown. Drop waits for the consumer, so keep the
            // refreshed file-key mappings but discard all interval/thread/IRP state whose
            // continuity was invalidated by the observed loss.
            self.session = None;
            self.sink.reset_session_state_preserving_files();
            self.totals.clear();
            self.interval_start_processes = None;
            self.last_health = etw::Health::default();
            return Collection::degraded(Vec::new());
        }
        self.last_health = health;

        let interval = self.sink.take_interval();
        let previous_processes = self.interval_start_processes.take();
        self.interval_start_processes = current_processes.clone();
        let Some(current_processes) = current_processes else {
            return collection_for_status(processes.status(), self.rows());
        };
        let Some(previous_processes) = previous_processes else {
            self.retain_active(&current_processes);
            let rows = self.rows();
            return if self.topology_failures > 0
                || self.sink.file_map_incomplete()
                || disks.counters.status() == CollectionStatus::Degraded
            {
                Collection::degraded(rows)
            } else {
                Collection::available(rows)
            };
        };

        let mut unresolved_disks = 0_usize;
        accumulate_interval(
            &mut self.totals,
            interval.bytes,
            &previous_processes,
            &current_processes,
            &disks.identities,
            &mut unresolved_disks,
        );
        self.retain_active(&current_processes);
        let rows = self.rows();
        let degraded = interval.stats.ambiguous_volume_ios > 0
            || interval.stats.unmapped_file_ios > 0
            || interval.stats.unresolved_local_files > 0
            || interval.stats.invalid_completions > 0
            || interval.stats.state_overflowed
            || unresolved_disks > 0
            || self.topology_failures > 0
            || self.sink.file_map_incomplete()
            || disks.counters.status() == CollectionStatus::Degraded;
        if degraded {
            report_issue(&mut notes, || {
                format!(
                    "ETW disk attribution degraded: {} ambiguous-volume I/Os, {} unmapped-file I/Os, {} unresolved local files, {} invalid completions, {} unresolved disk identities, {} volume-topology failures, file_map_incomplete={}, state_overflowed={}",
                    interval.stats.ambiguous_volume_ios,
                    interval.stats.unmapped_file_ios,
                    interval.stats.unresolved_local_files,
                    interval.stats.invalid_completions,
                    unresolved_disks,
                    self.topology_failures,
                    self.sink.file_map_incomplete(),
                    interval.stats.state_overflowed
                )
            });
            Collection::degraded(rows)
        } else {
            Collection::available(rows)
        }
    }

    fn refresh_topology_if_needed(&mut self) -> io::Result<()> {
        if self
            .topology_refreshed_at
            .is_some_and(|at| at.elapsed() < TOPOLOGY_REFRESH_INTERVAL)
        {
            return if self.topology_ready {
                Ok(())
            } else {
                Err(io::Error::other(
                    "volume topology unavailable; refresh retry is rate-limited",
                ))
            };
        }
        let now = Instant::now();
        let topology = match native::volume_disk_bindings() {
            Ok(topology) => topology,
            Err(error) => {
                self.topology_refreshed_at = Some(now);
                self.topology_failures = self.topology_failures.saturating_add(1);
                return if self.topology_ready {
                    Ok(())
                } else {
                    Err(error)
                };
            }
        };
        self.topology_failures = topology.failures;
        let routes = topology
            .bindings
            .into_iter()
            .filter(|binding| !binding.disk_numbers.is_empty())
            .map(|binding| VolumeRoute {
                nt_path_lower: Arc::<str>::from(binding.nt_path.to_ascii_lowercase()),
                target: if binding.disk_numbers.len() == 1 {
                    FileTarget::SingleDisk(binding.disk_numbers[0])
                } else {
                    FileTarget::Ambiguous
                },
            })
            .collect();
        self.sink.update_topology(routes);
        self.topology_refreshed_at = Some(now);
        self.topology_ready = true;
        Ok(())
    }

    fn fail_session(
        &mut self,
        error: io::Error,
        notes: &mut Option<&mut Vec<String>>,
    ) -> Collection<Vec<ProcessDiskIoCounter>> {
        let reason = unavailable_from_etw_io(&error);
        report_issue(notes, || format!("ETW disk attribution failed: {error}"));
        self.session = None;
        self.retry_after = Some((Instant::now() + RETRY_INTERVAL, reason));
        self.interval_start_processes = None;
        self.sink.reset_epoch();
        self.seeded = false;
        self.totals.clear();
        Collection::unavailable(reason)
    }

    fn retain_active(&mut self, current: &HashMap<u32, ProcessInstanceId>) {
        let active: HashSet<_> = current.values().copied().collect();
        self.totals.retain(|key, _| active.contains(&key.process));
    }

    fn rows(&self) -> Vec<ProcessDiskIoCounter> {
        self.totals
            .iter()
            .map(|(key, total)| ProcessDiskIoCounter {
                process: key.process,
                disk_id: key.disk_id.clone(),
                device: total.device.clone(),
                read_bytes: total.read_bytes,
                write_bytes: total.write_bytes,
            })
            .collect()
    }
}

fn unavailable_from_etw_io(error: &io::Error) -> cclover_core::model::CollectionUnavailable {
    if error.raw_os_error() == Some(5) {
        cclover_core::model::CollectionUnavailable::PermissionDenied
    } else {
        unavailable_from_io(error)
    }
}

fn collection_for_status(
    status: CollectionStatus,
    rows: Vec<ProcessDiskIoCounter>,
) -> Collection<Vec<ProcessDiskIoCounter>> {
    match status {
        CollectionStatus::Available => Collection::available(rows),
        CollectionStatus::Degraded => Collection::degraded(rows),
        CollectionStatus::Unavailable(reason) => Collection::unavailable(reason),
    }
}

fn complete_process_map(
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

fn stable_process_identity(
    pid: u32,
    previous: &HashMap<u32, ProcessInstanceId>,
    current: &HashMap<u32, ProcessInstanceId>,
) -> Option<ProcessInstanceId> {
    let previous = previous.get(&pid)?;
    let current = current.get(&pid)?;
    (previous == current).then_some(*current)
}

fn volume_path_matches(path: &str, volume: &str) -> bool {
    path == volume
        || path
            .strip_prefix(volume)
            .is_some_and(|rest| rest.starts_with('\\'))
}

fn accumulate_interval(
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

#[cfg(test)]
mod tests {
    use super::*;

    fn process(pid: u32, birth_marker: u64) -> ProcessInstanceId {
        ProcessInstanceId { pid, birth_marker }
    }

    fn route(path: &str, target: FileTarget) -> VolumeRoute {
        VolumeRoute {
            nt_path_lower: Arc::<str>::from(path.to_ascii_lowercase()),
            target,
        }
    }

    #[test]
    fn completed_logical_io_is_counted_on_single_disk_volume() {
        let mut state = SinkState::new();
        state.update_topology(vec![route(
            r"\Device\HarddiskVolume3",
            FileTarget::SingleDisk(7),
        )]);
        state.threads.insert(10, 42);
        state.remember_file(100, r"\Device\HarddiskVolume3\data.bin");
        state.start_io(500, 10, 0, 100, Direction::Read, 4096, 0);
        state.finish_io(500, 1024, 0);

        assert_eq!(state.interval.bytes.len(), 1);
        assert_eq!(
            state.interval.bytes[&IntervalKey {
                pid: 42,
                disk_number: 7,
                direction: DirectionKey::Read,
            }],
            1024
        );
    }

    #[test]
    fn paging_io_is_not_product_logical_io() {
        let mut state = SinkState::new();
        state.update_topology(vec![route(
            r"\Device\HarddiskVolume3",
            FileTarget::SingleDisk(7),
        )]);
        state.threads.insert(10, 42);
        state.remember_file(100, r"\Device\HarddiskVolume3\pagefile.sys");
        state.start_io(500, 10, 0, 100, Direction::Write, 4096, IRP_PAGING_IO);
        state.finish_io(500, 4096, 0);
        assert!(state.interval.bytes.is_empty());
    }

    #[test]
    fn ambiguous_volume_is_skipped_and_degrades_interval() {
        let mut state = SinkState::new();
        state.update_topology(vec![route(
            r"\Device\HarddiskVolume9",
            FileTarget::Ambiguous,
        )]);
        state.threads.insert(10, 42);
        state.remember_file(100, r"\Device\HarddiskVolume9\stripe.bin");
        state.start_io(500, 10, 0, 100, Direction::Write, 4096, 0);
        state.finish_io(500, 4096, 0);
        assert!(state.interval.bytes.is_empty());
        assert_eq!(state.interval.stats.ambiguous_volume_ios, 1);
    }

    #[test]
    fn process_identity_must_be_stable_across_interval() {
        let previous = HashMap::from([(42, process(42, 100))]);
        let current = HashMap::from([(42, process(42, 200))]);
        assert_eq!(stable_process_identity(42, &previous, &current), None);
    }

    #[test]
    fn failed_or_impossible_completion_is_not_counted() {
        let mut state = SinkState::new();
        state.update_topology(vec![route(
            r"\Device\HarddiskVolume3",
            FileTarget::SingleDisk(7),
        )]);
        state.threads.insert(10, 42);
        state.remember_file(100, r"\Device\HarddiskVolume3\data.bin");
        state.start_io(500, 10, 0, 100, Direction::Read, 1024, 0);
        state.finish_io(500, 2048, 0);
        assert!(state.interval.bytes.is_empty());
        assert_eq!(state.interval.stats.invalid_completions, 1);
    }

    #[test]
    fn volume_prefix_requires_path_boundary() {
        assert!(volume_path_matches(
            r"\device\harddiskvolume1\file.bin",
            r"\device\harddiskvolume1"
        ));
        assert!(!volume_path_matches(
            r"\device\harddiskvolume10\file.bin",
            r"\device\harddiskvolume1"
        ));
    }

    #[test]
    fn file_object_and_file_key_have_independent_lifetimes() {
        let sink = Sink::new();
        {
            let mut state = sink.state.lock().unwrap();
            state.update_topology(vec![route(
                r"\Device\HarddiskVolume3",
                FileTarget::SingleDisk(7),
            )]);
            state.remember_file(100, r"\Device\HarddiskVolume3\object.bin");
            state.remember_file(200, r"\Device\HarddiskVolume3\key.bin");
        }

        sink.on_event(Event::FileObjectEnd { file_object: 100 });
        {
            let state = sink.state.lock().unwrap();
            assert!(!state.files.contains_key(&100));
            assert!(state.files.contains_key(&200));
        }

        sink.on_event(Event::FileKeyEnd { key: 200 });
        assert!(!sink.state.lock().unwrap().files.contains_key(&200));
    }

    #[test]
    fn session_reset_preserves_rundown_file_mapping_only() {
        let mut state = SinkState::new();
        state.update_topology(vec![route(
            r"\Device\HarddiskVolume3",
            FileTarget::SingleDisk(7),
        )]);
        state.remember_file(100, r"\Device\HarddiskVolume3\data.bin");
        state.threads.insert(10, 42);
        state.pending.insert(
            500,
            PendingIo {
                pid: 42,
                target: FileTarget::SingleDisk(7),
                direction: DirectionKey::Read,
                requested_bytes: 1,
            },
        );

        state.reset_session_state_preserving_files();

        assert!(state.files.contains_key(&100));
        assert!(state.threads.is_empty());
        assert!(state.pending.is_empty());
        assert!(state.interval.bytes.is_empty());
    }

    #[test]
    fn unmapped_file_io_is_explicit_degradation_evidence() {
        let mut state = SinkState::new();
        state.threads.insert(10, 42);
        state.start_io(500, 10, 100, 200, Direction::Read, 4096, 0);
        assert_eq!(state.interval.stats.unmapped_file_ios, 1);
        assert!(state.pending.is_empty());
    }
}
