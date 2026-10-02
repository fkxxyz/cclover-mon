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

mod collector;
mod correlation;
mod support;
#[cfg(feature = "windows-etw-validation")]
mod validation;

pub(super) use collector::Collector;
use correlation::Sink;
use support::*;
#[cfg(feature = "windows-etw-validation")]
pub(super) fn run_native_semantic_validation() -> Result<(), String> {
    validation::run_native_semantic_validation()
}

#[cfg(test)]
mod tests;
