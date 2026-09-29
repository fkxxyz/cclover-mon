use std::collections::{BTreeMap, VecDeque};
use std::time::Instant;

#[derive(Clone, Debug, Default)]
pub struct MemorySnapshot {
    pub used_bytes: u64,
    pub total_bytes: u64,
    pub swap_used_bytes: u64,
    pub swap_total_bytes: u64,
}

#[derive(Clone, Debug)]
pub struct ProcessCpuUsage {
    pub name: String,
    pub percent: f64,
}

#[derive(Clone, Debug)]
pub struct ProcessMemoryUsage {
    pub name: String,
    pub bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProcessInstanceId {
    pub pid: u32,
    pub birth_marker: u64,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NetworkId(String);

impl NetworkId {
    pub fn from_opaque_key(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    pub fn as_opaque_key(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DiskId(String);

impl DiskId {
    pub fn from_opaque_key(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    pub fn as_opaque_key(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GpuId(String);

impl GpuId {
    pub fn from_opaque_key(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    pub fn as_opaque_key(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub struct NetworkSnapshot {
    pub id: NetworkId,
    pub name: String,
    pub down_bytes_per_sec: f64,
    pub up_bytes_per_sec: f64,
}

#[derive(Clone, Debug)]
pub struct DiskSnapshot {
    pub id: DiskId,
    pub name: String,
    pub bytes_per_sec: f64,
}

#[derive(Clone, Debug)]
pub struct TemperatureSnapshot {
    pub id: String,
    pub name: String,
    pub celsius: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GpuSnapshot {
    pub id: GpuId,
    pub name: String,
    pub utilization_percent: Option<f64>,
    pub memory_used_bytes: Option<u64>,
    pub memory_total_bytes: Option<u64>,
    pub temperature_celsius: Option<f64>,
    pub power_watts: Option<f64>,
    pub core_clock_mhz: Option<u64>,
    pub fan_percent: Option<f64>,
    pub fan_rpm: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessDiskIoCounter {
    pub process: ProcessInstanceId,
    pub disk_id: DiskId,
    pub device: String,
    pub read_bytes: u64,
    pub write_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessNetworkIoCounter {
    pub process: ProcessInstanceId,
    pub network_id: NetworkId,
    pub interface: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProcessDiskIo {
    pub process: ProcessInstanceId,
    pub name: Option<String>,
    pub disk_id: DiskId,
    pub device: String,
    pub read_bytes_per_sec: f64,
    pub write_bytes_per_sec: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProcessNetworkIo {
    pub process: ProcessInstanceId,
    pub name: Option<String>,
    pub network_id: NetworkId,
    pub interface: String,
    pub rx_bytes_per_sec: f64,
    pub tx_bytes_per_sec: f64,
}

#[derive(Clone, Debug, Default)]
pub struct SystemSnapshot {
    pub cpu_percent: Collection<f64>,
    pub memory: Collection<MemorySnapshot>,
    pub top_cpu: Collection<Vec<ProcessCpuUsage>>,
    pub top_memory: Collection<Vec<ProcessMemoryUsage>>,
    pub networks: Collection<Vec<NetworkSnapshot>>,
    pub disks: Collection<Vec<DiskSnapshot>>,
    pub process_disk_io: Collection<Vec<ProcessDiskIo>>,
    pub process_network_io: Collection<Vec<ProcessNetworkIo>>,
    pub temperatures: Collection<Vec<TemperatureSnapshot>>,
    pub gpus: Collection<Vec<GpuSnapshot>>,
}

#[derive(Clone, Debug, Default)]
pub struct NetworkDirectionHistory {
    pub down: VecDeque<f64>,
    pub up: VecDeque<f64>,
}

#[derive(Clone, Debug, Default)]
pub struct MonitorHistory {
    pub cpu: VecDeque<f64>,
    pub memory_used: VecDeque<f64>,
    pub swap_used: VecDeque<f64>,
    pub gpu_utilization: BTreeMap<GpuId, VecDeque<f64>>,
    pub gpu_memory_used: BTreeMap<GpuId, VecDeque<f64>>,
    pub gpu_temperature: BTreeMap<GpuId, VecDeque<f64>>,
    pub networks: BTreeMap<NetworkId, NetworkDirectionHistory>,
    pub disks: BTreeMap<DiskId, VecDeque<f64>>,
    pub temperatures: BTreeMap<String, VecDeque<f64>>,
}

#[derive(Clone, Debug, Default)]
pub struct MonitorState {
    pub snapshot: SystemSnapshot,
    pub history: MonitorHistory,
    pub history_capacity: usize,
}

#[derive(Clone, Debug)]
pub struct CpuCounter {
    pub total_time_units: u64,
    pub idle_time_units: u64,
    pub logical_cpu_count: usize,
}

#[derive(Clone, Debug)]
pub struct ProcessCounter {
    pub process: ProcessInstanceId,
    pub name: String,
    pub cpu_time_units: u64,
    pub rss_bytes: u64,
}

#[derive(Clone, Debug)]
pub struct NetworkCounter {
    pub id: NetworkId,
    pub name: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

#[derive(Clone, Debug)]
pub struct DiskCounter {
    pub id: DiskId,
    pub name: String,
    pub read_bytes: u64,
    pub write_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollectionUnavailable {
    Unsupported,
    Disabled,
    PermissionDenied,
    Unavailable,
    InvalidData,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollectionStatus {
    Available,
    Degraded,
    Unavailable(CollectionUnavailable),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Collection<T> {
    Available(T),
    Degraded(T),
    Unavailable(CollectionUnavailable),
}

impl<T> Collection<T> {
    pub fn available(value: T) -> Self {
        Self::Available(value)
    }

    pub fn degraded(value: T) -> Self {
        Self::Degraded(value)
    }

    pub fn unavailable(reason: CollectionUnavailable) -> Self {
        Self::Unavailable(reason)
    }

    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Available(value) | Self::Degraded(value) => Some(value),
            Self::Unavailable(_) => None,
        }
    }

    pub fn into_value(self) -> Option<T> {
        match self {
            Self::Available(value) | Self::Degraded(value) => Some(value),
            Self::Unavailable(_) => None,
        }
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Collection<U> {
        match self {
            Self::Available(value) => Collection::Available(f(value)),
            Self::Degraded(value) => Collection::Degraded(f(value)),
            Self::Unavailable(reason) => Collection::Unavailable(reason),
        }
    }

    pub fn is_observable(&self) -> bool {
        self.value().is_some()
    }

    pub fn status(&self) -> CollectionStatus {
        match self {
            Self::Available(_) => CollectionStatus::Available,
            Self::Degraded(_) => CollectionStatus::Degraded,
            Self::Unavailable(reason) => CollectionStatus::Unavailable(*reason),
        }
    }
}

impl<T> Default for Collection<T> {
    fn default() -> Self {
        Self::Unavailable(CollectionUnavailable::Unavailable)
    }
}

#[derive(Clone, Debug)]
pub struct RawSnapshot {
    pub collected_at: Instant,
    pub cpu: Collection<CpuCounter>,
    pub memory: Collection<MemorySnapshot>,
    pub processes: Collection<Vec<ProcessCounter>>,
    pub networks: Collection<Vec<NetworkCounter>>,
    pub disks: Collection<Vec<DiskCounter>>,
    pub process_disk_io: Collection<Vec<ProcessDiskIoCounter>>,
    pub process_network_io: Collection<Vec<ProcessNetworkIoCounter>>,
    pub temperatures: Collection<Vec<TemperatureSnapshot>>,
    pub gpus: Collection<Vec<GpuSnapshot>>,
}

impl Default for RawSnapshot {
    fn default() -> Self {
        Self::unavailable(Instant::now(), CollectionUnavailable::Unavailable)
    }
}

impl RawSnapshot {
    pub fn unavailable(collected_at: Instant, reason: CollectionUnavailable) -> Self {
        Self {
            collected_at,
            cpu: Collection::unavailable(reason),
            memory: Collection::unavailable(reason),
            processes: Collection::unavailable(reason),
            networks: Collection::unavailable(reason),
            disks: Collection::unavailable(reason),
            process_disk_io: Collection::unavailable(reason),
            process_network_io: Collection::unavailable(reason),
            temperatures: Collection::unavailable(reason),
            gpus: Collection::unavailable(reason),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_observation_is_distinct_from_unavailability() {
        let empty = Collection::<Vec<u8>>::available(Vec::new());
        let unavailable = Collection::<Vec<u8>>::unavailable(CollectionUnavailable::Unavailable);

        assert!(empty.is_observable());
        assert_eq!(empty.value().unwrap().len(), 0);
        assert!(!unavailable.is_observable());
    }

    #[test]
    fn mapping_preserves_degradation_state() {
        let outcome = Collection::degraded(vec![1, 2]).map(|values| values.len());
        assert_eq!(outcome, Collection::Degraded(2));
    }
}
