use std::collections::{BTreeMap, VecDeque};

use cclover_core::model::{Collection, CollectionUnavailable};
use serde::Serialize;

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct ApiV1State {
    pub(super) snapshot: ApiV1Snapshot,
    pub(super) history: ApiV1History,
    pub(super) history_capacity: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ApiV1Slice {
    Cpu,
    Memory,
    Disks,
    Networks,
    Temperatures,
    Fans,
    Gpus,
    Processes,
    HistoryCpu,
    HistoryMemory,
    HistoryGpus,
    HistoryDisks,
    HistoryNetworks,
    HistoryTemperatures,
    HistoryFans,
}

/// Ranked process rows the default v1 views carry for CPU and memory.
///
/// This is a wire contract number and is deliberately independent from any dashboard card row
/// count: the API states how many rows a default payload contains, not how a panel is laid out.
pub(super) const PROCESS_ROWS: usize = 8;

/// Ranked process rows the default v1 views carry per device for disk and network I/O.
pub(super) const PROCESS_IO_ROWS: usize = 3;

#[derive(Clone, Debug, Default, Serialize)]
pub(super) struct ApiV1Snapshot {
    pub(super) cpu_percent: ApiV1Collection<f64>,
    pub(super) memory: ApiV1Collection<ApiV1MemorySnapshot>,
    pub(super) top_cpu: ApiV1Collection<Vec<ApiV1ProcessCpu>>,
    pub(super) top_memory: ApiV1Collection<Vec<ApiV1ProcessMemory>>,
    pub(super) networks: ApiV1Collection<Vec<ApiV1NetworkSnapshot>>,
    pub(super) disks: ApiV1Collection<Vec<ApiV1DiskSnapshot>>,
    pub(super) process_disk_io: ApiV1Collection<Vec<ApiV1ProcessDiskIo>>,
    pub(super) process_network_io: ApiV1Collection<Vec<ApiV1ProcessNetworkIo>>,
    pub(super) temperatures: ApiV1Collection<Vec<ApiV1TemperatureSnapshot>>,
    pub(super) fans: ApiV1Collection<Vec<ApiV1FanSnapshot>>,
    pub(super) gpus: ApiV1Collection<Vec<ApiV1GpuSnapshot>>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "status", content = "value", rename_all = "snake_case")]
pub(super) enum ApiV1Collection<T> {
    Available(T),
    Degraded(T),
    Unavailable(ApiV1Unavailable),
}

impl<T> Default for ApiV1Collection<T> {
    fn default() -> Self {
        Self::Unavailable(ApiV1Unavailable::Unavailable)
    }
}

impl<T> ApiV1Collection<T> {
    pub(super) fn from_core<S>(source: &Collection<S>, map: impl FnOnce(&S) -> T) -> Self {
        match source {
            Collection::Available(value) => Self::Available(map(value)),
            Collection::Degraded(value) => Self::Degraded(map(value)),
            Collection::Unavailable(reason) => Self::Unavailable((*reason).into()),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ApiV1Unavailable {
    Unsupported,
    Disabled,
    PermissionDenied,
    Unavailable,
    InvalidData,
}

impl From<CollectionUnavailable> for ApiV1Unavailable {
    fn from(value: CollectionUnavailable) -> Self {
        match value {
            CollectionUnavailable::Unsupported => Self::Unsupported,
            CollectionUnavailable::Disabled => Self::Disabled,
            CollectionUnavailable::PermissionDenied => Self::PermissionDenied,
            CollectionUnavailable::Unavailable => Self::Unavailable,
            CollectionUnavailable::InvalidData => Self::InvalidData,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub(super) struct ApiV1MemorySnapshot {
    pub(super) used_bytes: u64,
    pub(super) total_bytes: u64,
    pub(super) swap_used_bytes: u64,
    pub(super) swap_total_bytes: u64,
}

/// Identity of the process a ranked row refers to.
///
/// `pid` alone is insufficient because operating systems reuse it. `instance` is an opaque
/// per-instance marker: it is stable for one process instance and differs when a pid is reused.
/// It is a decimal string because the underlying platform marker does not fit the exactly
/// representable JSON number range on every platform, and because it must not be read as a time.
#[derive(Clone, Debug, Serialize)]
pub(super) struct ApiV1ProcessRef {
    pub(super) pid: u32,
    pub(super) instance: String,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct ApiV1ProcessCpu {
    pub(super) process: ApiV1ProcessRef,
    pub(super) name: Option<String>,
    pub(super) percent: f64,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct ApiV1ProcessMemory {
    pub(super) process: ApiV1ProcessRef,
    pub(super) name: Option<String>,
    pub(super) bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct ApiV1NetworkSnapshot {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) down_bytes_per_sec: f64,
    pub(super) up_bytes_per_sec: f64,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct ApiV1DiskSnapshot {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) bytes_per_sec: f64,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct ApiV1ProcessDiskIo {
    pub(super) process: ApiV1ProcessRef,
    pub(super) name: Option<String>,
    pub(super) disk_id: String,
    pub(super) device: String,
    pub(super) read_bytes_per_sec: f64,
    pub(super) write_bytes_per_sec: f64,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct ApiV1ProcessNetworkIo {
    pub(super) process: ApiV1ProcessRef,
    pub(super) name: Option<String>,
    pub(super) network_id: String,
    pub(super) interface: String,
    pub(super) rx_bytes_per_sec: f64,
    pub(super) tx_bytes_per_sec: f64,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct ApiV1TemperatureSnapshot {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) celsius: f64,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct ApiV1FanSnapshot {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) rpm: u64,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct ApiV1GpuSnapshot {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) utilization_percent: Option<f64>,
    pub(super) memory_used_bytes: Option<u64>,
    pub(super) memory_total_bytes: Option<u64>,
    pub(super) temperature_celsius: Option<f64>,
    pub(super) power_watts: Option<f64>,
    pub(super) core_clock_mhz: Option<u64>,
    pub(super) fan_percent: Option<f64>,
    pub(super) fan_rpm: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub(super) struct ApiV1DirectionHistory {
    pub(super) down: VecDeque<f64>,
    pub(super) up: VecDeque<f64>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub(super) struct ApiV1History {
    pub(super) cpu: VecDeque<f64>,
    pub(super) memory_used: VecDeque<f64>,
    pub(super) swap_used: VecDeque<f64>,
    pub(super) gpu_utilization: BTreeMap<String, VecDeque<f64>>,
    pub(super) gpu_memory_used: BTreeMap<String, VecDeque<f64>>,
    pub(super) gpu_temperature: BTreeMap<String, VecDeque<f64>>,
    pub(super) networks: BTreeMap<String, ApiV1DirectionHistory>,
    pub(super) disks: BTreeMap<String, VecDeque<f64>>,
    pub(super) temperatures: BTreeMap<String, VecDeque<f64>>,
    pub(super) fans: BTreeMap<String, VecDeque<f64>>,
}

#[derive(Serialize)]
pub(super) struct ApiV1Cpu<'a> {
    pub(super) cpu_percent: &'a ApiV1Collection<f64>,
    pub(super) top_cpu: &'a ApiV1Collection<Vec<ApiV1ProcessCpu>>,
}

#[derive(Serialize)]
pub(super) struct ApiV1Memory<'a> {
    pub(super) memory: &'a ApiV1Collection<ApiV1MemorySnapshot>,
    pub(super) top_memory: &'a ApiV1Collection<Vec<ApiV1ProcessMemory>>,
}

#[derive(Serialize)]
pub(super) struct ApiV1Disks<'a> {
    pub(super) disks: &'a ApiV1Collection<Vec<ApiV1DiskSnapshot>>,
    pub(super) process_disk_io: &'a ApiV1Collection<Vec<ApiV1ProcessDiskIo>>,
}

#[derive(Serialize)]
pub(super) struct ApiV1Networks<'a> {
    pub(super) networks: &'a ApiV1Collection<Vec<ApiV1NetworkSnapshot>>,
    pub(super) process_network_io: &'a ApiV1Collection<Vec<ApiV1ProcessNetworkIo>>,
}

#[derive(Serialize)]
pub(super) struct ApiV1Temperatures<'a> {
    pub(super) temperatures: &'a ApiV1Collection<Vec<ApiV1TemperatureSnapshot>>,
}

#[derive(Serialize)]
pub(super) struct ApiV1Fans<'a> {
    pub(super) fans: &'a ApiV1Collection<Vec<ApiV1FanSnapshot>>,
}

#[derive(Serialize)]
pub(super) struct ApiV1Gpus<'a> {
    pub(super) gpus: &'a ApiV1Collection<Vec<ApiV1GpuSnapshot>>,
}

#[derive(Serialize)]
pub(super) struct ApiV1Processes<'a> {
    pub(super) top_cpu: &'a ApiV1Collection<Vec<ApiV1ProcessCpu>>,
    pub(super) top_memory: &'a ApiV1Collection<Vec<ApiV1ProcessMemory>>,
    pub(super) disk_io: &'a ApiV1Collection<Vec<ApiV1ProcessDiskIo>>,
    pub(super) network_io: &'a ApiV1Collection<Vec<ApiV1ProcessNetworkIo>>,
}

#[derive(Serialize)]
pub(super) struct ApiV1CpuHistory<'a> {
    pub(super) history_capacity: usize,
    pub(super) cpu: &'a VecDeque<f64>,
}

#[derive(Serialize)]
pub(super) struct ApiV1MemoryHistory<'a> {
    pub(super) history_capacity: usize,
    pub(super) memory_used: &'a VecDeque<f64>,
    pub(super) swap_used: &'a VecDeque<f64>,
}

#[derive(Serialize)]
pub(super) struct ApiV1GpuHistory<'a> {
    pub(super) history_capacity: usize,
    pub(super) utilization: &'a BTreeMap<String, VecDeque<f64>>,
    pub(super) memory_used: &'a BTreeMap<String, VecDeque<f64>>,
    pub(super) temperature: &'a BTreeMap<String, VecDeque<f64>>,
}

#[derive(Serialize)]
pub(super) struct ApiV1DisksHistory<'a> {
    pub(super) history_capacity: usize,
    pub(super) disks: &'a BTreeMap<String, VecDeque<f64>>,
}

#[derive(Serialize)]
pub(super) struct ApiV1NetworksHistory<'a> {
    pub(super) history_capacity: usize,
    pub(super) networks: &'a BTreeMap<String, ApiV1DirectionHistory>,
}

#[derive(Serialize)]
pub(super) struct ApiV1TemperaturesHistory<'a> {
    pub(super) history_capacity: usize,
    pub(super) temperatures: &'a BTreeMap<String, VecDeque<f64>>,
}

#[derive(Serialize)]
pub(super) struct ApiV1FansHistory<'a> {
    pub(super) history_capacity: usize,
    pub(super) fans: &'a BTreeMap<String, VecDeque<f64>>,
}
