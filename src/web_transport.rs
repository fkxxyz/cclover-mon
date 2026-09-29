use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};

use crate::core::model::{
    Collection, CollectionUnavailable, DiskId, DiskSnapshot, GpuId, GpuMemorySnapshot,
    MemorySnapshot, MonitorHistory, MonitorState, NetworkDirectionHistory, NetworkId,
    NetworkSnapshot, ProcessCpuUsage, ProcessDiskIo, ProcessInstanceId, ProcessMemoryUsage,
    ProcessNetworkIo, SystemSnapshot, TemperatureSnapshot,
};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct WebMonitorState {
    snapshot: WebSystemSnapshot,
    history: WebMonitorHistory,
    history_capacity: usize,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "http"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WebApiSlice {
    Cpu,
    Memory,
    Disks,
    Networks,
    Temperatures,
    GpuMemory,
    Processes,
    HistoryCpu,
    HistoryMemory,
    HistoryGpuMemory,
    HistoryDisks,
    HistoryNetworks,
    HistoryTemperatures,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "http"))]
impl WebMonitorState {
    pub(crate) fn serialize_api_slice(&self, slice: WebApiSlice) -> serde_json::Result<String> {
        match slice {
            WebApiSlice::Cpu => serde_json::to_string(&WebCpuApi {
                cpu_percent: &self.snapshot.cpu_percent,
                top_cpu: &self.snapshot.top_cpu,
            }),
            WebApiSlice::Memory => serde_json::to_string(&WebMemoryApi {
                memory: &self.snapshot.memory,
                top_memory: &self.snapshot.top_memory,
            }),
            WebApiSlice::Disks => serde_json::to_string(&WebDisksApi {
                disks: &self.snapshot.disks,
                process_disk_io: &self.snapshot.process_disk_io,
            }),
            WebApiSlice::Networks => serde_json::to_string(&WebNetworksApi {
                networks: &self.snapshot.networks,
                process_network_io: &self.snapshot.process_network_io,
            }),
            WebApiSlice::Temperatures => serde_json::to_string(&WebTemperaturesApi {
                temperatures: &self.snapshot.temperatures,
            }),
            WebApiSlice::GpuMemory => serde_json::to_string(&WebGpuMemoryApi {
                gpu_memory: &self.snapshot.gpu_memory,
            }),
            WebApiSlice::Processes => serde_json::to_string(&WebProcessesApi {
                top_cpu: &self.snapshot.top_cpu,
                top_memory: &self.snapshot.top_memory,
                disk_io: &self.snapshot.process_disk_io,
                network_io: &self.snapshot.process_network_io,
            }),
            WebApiSlice::HistoryCpu => serde_json::to_string(&WebCpuHistoryApi {
                history_capacity: self.history_capacity,
                cpu: &self.history.cpu,
            }),
            WebApiSlice::HistoryMemory => serde_json::to_string(&WebMemoryHistoryApi {
                history_capacity: self.history_capacity,
                memory_used: &self.history.memory_used,
                swap_used: &self.history.swap_used,
            }),
            WebApiSlice::HistoryGpuMemory => serde_json::to_string(&WebGpuMemoryHistoryApi {
                history_capacity: self.history_capacity,
                gpu_memory_used: &self.history.gpu_memory_used,
            }),
            WebApiSlice::HistoryDisks => serde_json::to_string(&WebDisksHistoryApi {
                history_capacity: self.history_capacity,
                disks: &self.history.disks,
            }),
            WebApiSlice::HistoryNetworks => serde_json::to_string(&WebNetworksHistoryApi {
                history_capacity: self.history_capacity,
                networks: &self.history.networks,
            }),
            WebApiSlice::HistoryTemperatures => serde_json::to_string(&WebTemperaturesHistoryApi {
                history_capacity: self.history_capacity,
                temperatures: &self.history.temperatures,
            }),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct WebSystemSnapshot {
    cpu_percent: WebCollection<f64>,
    memory: WebCollection<WebMemorySnapshot>,
    top_cpu: WebCollection<Vec<WebProcessCpu>>,
    top_memory: WebCollection<Vec<WebProcessMemory>>,
    networks: WebCollection<Vec<WebNetworkSnapshot>>,
    disks: WebCollection<Vec<WebDiskSnapshot>>,
    process_disk_io: WebCollection<Vec<WebProcessDiskIo>>,
    process_network_io: WebCollection<Vec<WebProcessNetworkIo>>,
    temperatures: WebCollection<Vec<WebTemperatureSnapshot>>,
    gpu_memory: WebCollection<Vec<WebGpuMemorySnapshot>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "status", content = "value", rename_all = "snake_case")]
enum WebCollection<T> {
    Available(T),
    Degraded(T),
    Unavailable(WebCollectionUnavailable),
}

impl<T> Default for WebCollection<T> {
    fn default() -> Self {
        Self::Unavailable(WebCollectionUnavailable::Unavailable)
    }
}

impl<T> WebCollection<T> {
    fn from_core<S>(source: &Collection<S>, map: impl FnOnce(&S) -> T) -> Self {
        match source {
            Collection::Available(value) => Self::Available(map(value)),
            Collection::Degraded(value) => Self::Degraded(map(value)),
            Collection::Unavailable(reason) => Self::Unavailable((*reason).into()),
        }
    }

    fn into_core<U>(self, map: impl FnOnce(T) -> U) -> Collection<U> {
        match self {
            Self::Available(value) => Collection::Available(map(value)),
            Self::Degraded(value) => Collection::Degraded(map(value)),
            Self::Unavailable(reason) => Collection::Unavailable(reason.into()),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum WebCollectionUnavailable {
    Unsupported,
    Disabled,
    PermissionDenied,
    Unavailable,
    InvalidData,
}

impl From<CollectionUnavailable> for WebCollectionUnavailable {
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

impl From<WebCollectionUnavailable> for CollectionUnavailable {
    fn from(value: WebCollectionUnavailable) -> Self {
        match value {
            WebCollectionUnavailable::Unsupported => Self::Unsupported,
            WebCollectionUnavailable::Disabled => Self::Disabled,
            WebCollectionUnavailable::PermissionDenied => Self::PermissionDenied,
            WebCollectionUnavailable::Unavailable => Self::Unavailable,
            WebCollectionUnavailable::InvalidData => Self::InvalidData,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct WebMemorySnapshot {
    used_bytes: u64,
    total_bytes: u64,
    swap_used_bytes: u64,
    swap_total_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct WebProcessCpu {
    name: String,
    percent: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct WebProcessMemory {
    name: String,
    bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct WebNetworkSnapshot {
    id: String,
    name: String,
    down_bytes_per_sec: f64,
    up_bytes_per_sec: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct WebDiskSnapshot {
    id: String,
    name: String,
    bytes_per_sec: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct WebProcessDiskIo {
    pid: u32,
    birth_marker: u64,
    name: Option<String>,
    disk_id: String,
    device: String,
    read_bytes_per_sec: f64,
    write_bytes_per_sec: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct WebProcessNetworkIo {
    pid: u32,
    birth_marker: u64,
    name: Option<String>,
    network_id: String,
    interface: String,
    rx_bytes_per_sec: f64,
    tx_bytes_per_sec: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct WebTemperatureSnapshot {
    id: String,
    name: String,
    celsius: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct WebGpuMemorySnapshot {
    id: String,
    name: String,
    used_bytes: u64,
    total_bytes: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct WebDirectionHistory {
    down: VecDeque<f64>,
    up: VecDeque<f64>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct WebMonitorHistory {
    cpu: VecDeque<f64>,
    memory_used: VecDeque<f64>,
    swap_used: VecDeque<f64>,
    gpu_memory_used: BTreeMap<String, VecDeque<f64>>,
    networks: BTreeMap<String, WebDirectionHistory>,
    disks: BTreeMap<String, VecDeque<f64>>,
    temperatures: BTreeMap<String, VecDeque<f64>>,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "http"))]
#[derive(Serialize)]
struct WebCpuApi<'a> {
    cpu_percent: &'a WebCollection<f64>,
    top_cpu: &'a WebCollection<Vec<WebProcessCpu>>,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "http"))]
#[derive(Serialize)]
struct WebMemoryApi<'a> {
    memory: &'a WebCollection<WebMemorySnapshot>,
    top_memory: &'a WebCollection<Vec<WebProcessMemory>>,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "http"))]
#[derive(Serialize)]
struct WebDisksApi<'a> {
    disks: &'a WebCollection<Vec<WebDiskSnapshot>>,
    process_disk_io: &'a WebCollection<Vec<WebProcessDiskIo>>,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "http"))]
#[derive(Serialize)]
struct WebNetworksApi<'a> {
    networks: &'a WebCollection<Vec<WebNetworkSnapshot>>,
    process_network_io: &'a WebCollection<Vec<WebProcessNetworkIo>>,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "http"))]
#[derive(Serialize)]
struct WebTemperaturesApi<'a> {
    temperatures: &'a WebCollection<Vec<WebTemperatureSnapshot>>,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "http"))]
#[derive(Serialize)]
struct WebGpuMemoryApi<'a> {
    gpu_memory: &'a WebCollection<Vec<WebGpuMemorySnapshot>>,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "http"))]
#[derive(Serialize)]
struct WebProcessesApi<'a> {
    top_cpu: &'a WebCollection<Vec<WebProcessCpu>>,
    top_memory: &'a WebCollection<Vec<WebProcessMemory>>,
    disk_io: &'a WebCollection<Vec<WebProcessDiskIo>>,
    network_io: &'a WebCollection<Vec<WebProcessNetworkIo>>,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "http"))]
#[derive(Serialize)]
struct WebCpuHistoryApi<'a> {
    history_capacity: usize,
    cpu: &'a VecDeque<f64>,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "http"))]
#[derive(Serialize)]
struct WebMemoryHistoryApi<'a> {
    history_capacity: usize,
    memory_used: &'a VecDeque<f64>,
    swap_used: &'a VecDeque<f64>,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "http"))]
#[derive(Serialize)]
struct WebGpuMemoryHistoryApi<'a> {
    history_capacity: usize,
    gpu_memory_used: &'a BTreeMap<String, VecDeque<f64>>,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "http"))]
#[derive(Serialize)]
struct WebDisksHistoryApi<'a> {
    history_capacity: usize,
    disks: &'a BTreeMap<String, VecDeque<f64>>,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "http"))]
#[derive(Serialize)]
struct WebNetworksHistoryApi<'a> {
    history_capacity: usize,
    networks: &'a BTreeMap<String, WebDirectionHistory>,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "http"))]
#[derive(Serialize)]
struct WebTemperaturesHistoryApi<'a> {
    history_capacity: usize,
    temperatures: &'a BTreeMap<String, VecDeque<f64>>,
}

impl From<&MonitorState> for WebMonitorState {
    fn from(state: &MonitorState) -> Self {
        Self {
            snapshot: WebSystemSnapshot::from(&state.snapshot),
            history: WebMonitorHistory::from(&state.history),
            history_capacity: state.history_capacity,
        }
    }
}

impl From<WebMonitorState> for MonitorState {
    fn from(state: WebMonitorState) -> Self {
        Self {
            snapshot: state.snapshot.into(),
            history: state.history.into(),
            history_capacity: state.history_capacity,
        }
    }
}

impl From<&SystemSnapshot> for WebSystemSnapshot {
    fn from(snapshot: &SystemSnapshot) -> Self {
        Self {
            cpu_percent: WebCollection::from_core(&snapshot.cpu_percent, |value| *value),
            memory: WebCollection::from_core(&snapshot.memory, |value| {
                WebMemorySnapshot::from(value)
            }),
            top_cpu: WebCollection::from_core(&snapshot.top_cpu, |rows| {
                rows.iter().map(WebProcessCpu::from).collect()
            }),
            top_memory: WebCollection::from_core(&snapshot.top_memory, |rows| {
                rows.iter().map(WebProcessMemory::from).collect()
            }),
            networks: WebCollection::from_core(&snapshot.networks, |rows| {
                rows.iter().map(WebNetworkSnapshot::from).collect()
            }),
            disks: WebCollection::from_core(&snapshot.disks, |rows| {
                rows.iter().map(WebDiskSnapshot::from).collect()
            }),
            process_disk_io: WebCollection::from_core(&snapshot.process_disk_io, |rows| {
                rows.iter().map(WebProcessDiskIo::from).collect()
            }),
            process_network_io: WebCollection::from_core(&snapshot.process_network_io, |rows| {
                rows.iter().map(WebProcessNetworkIo::from).collect()
            }),
            temperatures: WebCollection::from_core(&snapshot.temperatures, |rows| {
                rows.iter().map(WebTemperatureSnapshot::from).collect()
            }),
            gpu_memory: WebCollection::from_core(&snapshot.gpu_memory, |rows| {
                rows.iter().map(WebGpuMemorySnapshot::from).collect()
            }),
        }
    }
}

impl From<WebSystemSnapshot> for SystemSnapshot {
    fn from(snapshot: WebSystemSnapshot) -> Self {
        Self {
            cpu_percent: snapshot.cpu_percent.into_core(|value| value),
            memory: snapshot.memory.into_core(MemorySnapshot::from),
            top_cpu: snapshot
                .top_cpu
                .into_core(|rows| rows.into_iter().map(ProcessCpuUsage::from).collect()),
            top_memory: snapshot
                .top_memory
                .into_core(|rows| rows.into_iter().map(ProcessMemoryUsage::from).collect()),
            networks: snapshot
                .networks
                .into_core(|rows| rows.into_iter().map(NetworkSnapshot::from).collect()),
            disks: snapshot
                .disks
                .into_core(|rows| rows.into_iter().map(DiskSnapshot::from).collect()),
            process_disk_io: snapshot
                .process_disk_io
                .into_core(|rows| rows.into_iter().map(ProcessDiskIo::from).collect()),
            process_network_io: snapshot
                .process_network_io
                .into_core(|rows| rows.into_iter().map(ProcessNetworkIo::from).collect()),
            temperatures: snapshot
                .temperatures
                .into_core(|rows| rows.into_iter().map(TemperatureSnapshot::from).collect()),
            gpu_memory: snapshot
                .gpu_memory
                .into_core(|rows| rows.into_iter().map(GpuMemorySnapshot::from).collect()),
        }
    }
}

impl From<&MemorySnapshot> for WebMemorySnapshot {
    fn from(value: &MemorySnapshot) -> Self {
        Self {
            used_bytes: value.used_bytes,
            total_bytes: value.total_bytes,
            swap_used_bytes: value.swap_used_bytes,
            swap_total_bytes: value.swap_total_bytes,
        }
    }
}

impl From<WebMemorySnapshot> for MemorySnapshot {
    fn from(value: WebMemorySnapshot) -> Self {
        Self {
            used_bytes: value.used_bytes,
            total_bytes: value.total_bytes,
            swap_used_bytes: value.swap_used_bytes,
            swap_total_bytes: value.swap_total_bytes,
        }
    }
}

impl From<&ProcessCpuUsage> for WebProcessCpu {
    fn from(value: &ProcessCpuUsage) -> Self {
        Self {
            name: value.name.clone(),
            percent: value.percent,
        }
    }
}

impl From<WebProcessCpu> for ProcessCpuUsage {
    fn from(value: WebProcessCpu) -> Self {
        Self {
            name: value.name,
            percent: value.percent,
        }
    }
}

impl From<&ProcessMemoryUsage> for WebProcessMemory {
    fn from(value: &ProcessMemoryUsage) -> Self {
        Self {
            name: value.name.clone(),
            bytes: value.bytes,
        }
    }
}

impl From<WebProcessMemory> for ProcessMemoryUsage {
    fn from(value: WebProcessMemory) -> Self {
        Self {
            name: value.name,
            bytes: value.bytes,
        }
    }
}

impl From<&NetworkSnapshot> for WebNetworkSnapshot {
    fn from(value: &NetworkSnapshot) -> Self {
        Self {
            id: value.id.as_opaque_key().to_owned(),
            name: value.name.clone(),
            down_bytes_per_sec: value.down_bytes_per_sec,
            up_bytes_per_sec: value.up_bytes_per_sec,
        }
    }
}

impl From<WebNetworkSnapshot> for NetworkSnapshot {
    fn from(value: WebNetworkSnapshot) -> Self {
        Self {
            id: NetworkId::from_opaque_key(value.id),
            name: value.name,
            down_bytes_per_sec: value.down_bytes_per_sec,
            up_bytes_per_sec: value.up_bytes_per_sec,
        }
    }
}

impl From<&DiskSnapshot> for WebDiskSnapshot {
    fn from(value: &DiskSnapshot) -> Self {
        Self {
            id: value.id.as_opaque_key().to_owned(),
            name: value.name.clone(),
            bytes_per_sec: value.bytes_per_sec,
        }
    }
}

impl From<WebDiskSnapshot> for DiskSnapshot {
    fn from(value: WebDiskSnapshot) -> Self {
        Self {
            id: DiskId::from_opaque_key(value.id),
            name: value.name,
            bytes_per_sec: value.bytes_per_sec,
        }
    }
}

impl From<&ProcessDiskIo> for WebProcessDiskIo {
    fn from(value: &ProcessDiskIo) -> Self {
        Self {
            pid: value.process.pid,
            birth_marker: value.process.birth_marker,
            name: value.name.clone(),
            disk_id: value.disk_id.as_opaque_key().to_owned(),
            device: value.device.clone(),
            read_bytes_per_sec: value.read_bytes_per_sec,
            write_bytes_per_sec: value.write_bytes_per_sec,
        }
    }
}

impl From<WebProcessDiskIo> for ProcessDiskIo {
    fn from(value: WebProcessDiskIo) -> Self {
        Self {
            process: ProcessInstanceId {
                pid: value.pid,
                birth_marker: value.birth_marker,
            },
            name: value.name,
            disk_id: DiskId::from_opaque_key(value.disk_id),
            device: value.device,
            read_bytes_per_sec: value.read_bytes_per_sec,
            write_bytes_per_sec: value.write_bytes_per_sec,
        }
    }
}

impl From<&ProcessNetworkIo> for WebProcessNetworkIo {
    fn from(value: &ProcessNetworkIo) -> Self {
        Self {
            pid: value.process.pid,
            birth_marker: value.process.birth_marker,
            name: value.name.clone(),
            network_id: value.network_id.as_opaque_key().to_owned(),
            interface: value.interface.clone(),
            rx_bytes_per_sec: value.rx_bytes_per_sec,
            tx_bytes_per_sec: value.tx_bytes_per_sec,
        }
    }
}

impl From<WebProcessNetworkIo> for ProcessNetworkIo {
    fn from(value: WebProcessNetworkIo) -> Self {
        Self {
            process: ProcessInstanceId {
                pid: value.pid,
                birth_marker: value.birth_marker,
            },
            name: value.name,
            network_id: NetworkId::from_opaque_key(value.network_id),
            interface: value.interface,
            rx_bytes_per_sec: value.rx_bytes_per_sec,
            tx_bytes_per_sec: value.tx_bytes_per_sec,
        }
    }
}

impl From<&TemperatureSnapshot> for WebTemperatureSnapshot {
    fn from(value: &TemperatureSnapshot) -> Self {
        Self {
            id: value.id.clone(),
            name: value.name.clone(),
            celsius: value.celsius,
        }
    }
}

impl From<WebTemperatureSnapshot> for TemperatureSnapshot {
    fn from(value: WebTemperatureSnapshot) -> Self {
        Self {
            id: value.id,
            name: value.name,
            celsius: value.celsius,
        }
    }
}

impl From<&GpuMemorySnapshot> for WebGpuMemorySnapshot {
    fn from(value: &GpuMemorySnapshot) -> Self {
        Self {
            id: value.id.as_opaque_key().to_owned(),
            name: value.name.clone(),
            used_bytes: value.used_bytes,
            total_bytes: value.total_bytes,
        }
    }
}

impl From<WebGpuMemorySnapshot> for GpuMemorySnapshot {
    fn from(value: WebGpuMemorySnapshot) -> Self {
        Self {
            id: GpuId::from_opaque_key(value.id),
            name: value.name,
            used_bytes: value.used_bytes,
            total_bytes: value.total_bytes,
        }
    }
}

impl From<&NetworkDirectionHistory> for WebDirectionHistory {
    fn from(value: &NetworkDirectionHistory) -> Self {
        Self {
            down: value.down.clone(),
            up: value.up.clone(),
        }
    }
}

impl From<WebDirectionHistory> for NetworkDirectionHistory {
    fn from(value: WebDirectionHistory) -> Self {
        Self {
            down: value.down,
            up: value.up,
        }
    }
}

impl From<&MonitorHistory> for WebMonitorHistory {
    fn from(history: &MonitorHistory) -> Self {
        Self {
            cpu: history.cpu.clone(),
            memory_used: history.memory_used.clone(),
            swap_used: history.swap_used.clone(),
            gpu_memory_used: history
                .gpu_memory_used
                .iter()
                .map(|(id, values)| (id.as_opaque_key().to_owned(), values.clone()))
                .collect(),
            networks: history
                .networks
                .iter()
                .map(|(id, value)| {
                    (
                        id.as_opaque_key().to_owned(),
                        WebDirectionHistory::from(value),
                    )
                })
                .collect(),
            disks: history
                .disks
                .iter()
                .map(|(id, values)| (id.as_opaque_key().to_owned(), values.clone()))
                .collect(),
            temperatures: history.temperatures.clone(),
        }
    }
}

impl From<WebMonitorHistory> for MonitorHistory {
    fn from(history: WebMonitorHistory) -> Self {
        Self {
            cpu: history.cpu,
            memory_used: history.memory_used,
            swap_used: history.swap_used,
            gpu_memory_used: history
                .gpu_memory_used
                .into_iter()
                .map(|(id, values)| (GpuId::from_opaque_key(id), values))
                .collect(),
            networks: history
                .networks
                .into_iter()
                .map(|(id, value)| {
                    (
                        NetworkId::from_opaque_key(id),
                        NetworkDirectionHistory::from(value),
                    )
                })
                .collect(),
            disks: history
                .disks
                .into_iter()
                .map(|(id, values)| (DiskId::from_opaque_key(id), values))
                .collect(),
            temperatures: history.temperatures,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_includes_process_attribution_for_shared_panel() {
        let mut state = MonitorState::default();
        let process = ProcessInstanceId {
            pid: 42,
            birth_marker: 7,
        };
        state.snapshot.process_disk_io = Collection::available(vec![ProcessDiskIo {
            process,
            name: Some("worker".to_owned()),
            disk_id: DiskId::from_opaque_key("disk-a"),
            device: "nvme0n1".to_owned(),
            read_bytes_per_sec: 1.0,
            write_bytes_per_sec: 2.0,
        }]);
        state.snapshot.process_network_io = Collection::available(vec![ProcessNetworkIo {
            process,
            name: Some("worker".to_owned()),
            network_id: NetworkId::from_opaque_key("network-a"),
            interface: "eth0".to_owned(),
            rx_bytes_per_sec: 3.0,
            tx_bytes_per_sec: 4.0,
        }]);

        let json = serde_json::to_string(&WebMonitorState::from(&state)).unwrap();
        let decoded = MonitorState::from(serde_json::from_str::<WebMonitorState>(&json).unwrap());

        assert!(json.contains("process_disk_io"));
        assert!(json.contains("process_network_io"));
        let disk = &decoded.snapshot.process_disk_io.value().unwrap()[0];
        assert_eq!(disk.process, process);
        assert_eq!(disk.name.as_deref(), Some("worker"));
        assert_eq!(disk.disk_id.as_opaque_key(), "disk-a");
        assert_eq!(disk.device, "nvme0n1");
        let network = &decoded.snapshot.process_network_io.value().unwrap()[0];
        assert_eq!(network.process, process);
        assert_eq!(network.name.as_deref(), Some("worker"));
        assert_eq!(network.network_id.as_opaque_key(), "network-a");
        assert_eq!(network.interface, "eth0");
    }

    #[test]
    fn transport_round_trip_preserves_panel_state() {
        let mut state = MonitorState::default();
        let network_id = NetworkId::from_opaque_key("network-a");
        state.snapshot.cpu_percent = Collection::available(37.5);
        state.snapshot.memory = Collection::available(MemorySnapshot {
            used_bytes: 10,
            total_bytes: 20,
            swap_used_bytes: 3,
            swap_total_bytes: 4,
        });
        state.snapshot.networks = Collection::available(vec![NetworkSnapshot {
            id: network_id.clone(),
            name: "eth0".to_owned(),
            down_bytes_per_sec: 12.0,
            up_bytes_per_sec: 5.0,
        }]);
        state.snapshot.gpu_memory = Collection::available(vec![GpuMemorySnapshot {
            id: GpuId::from_opaque_key("gpu-a"),
            name: "RTX Test".to_owned(),
            used_bytes: 4,
            total_bytes: 8,
        }]);
        state.history.cpu.push_back(11.0);
        state
            .history
            .gpu_memory_used
            .insert(GpuId::from_opaque_key("gpu-a"), VecDeque::from([2.0, 4.0]));
        state.history.networks.insert(
            network_id,
            NetworkDirectionHistory {
                down: VecDeque::from([7.0, 12.0]),
                up: VecDeque::from([3.0, 5.0]),
            },
        );
        state.history_capacity = 120;

        let json = serde_json::to_string(&WebMonitorState::from(&state)).unwrap();
        let decoded: WebMonitorState = serde_json::from_str(&json).unwrap();
        let decoded = MonitorState::from(decoded);

        assert_eq!(decoded.snapshot.cpu_percent.value().copied(), Some(37.5));
        assert_eq!(decoded.snapshot.memory.value().unwrap().used_bytes, 10);
        assert_eq!(decoded.snapshot.networks.value().unwrap()[0].name, "eth0");
        let gpu = &decoded.snapshot.gpu_memory.value().unwrap()[0];
        assert_eq!(gpu.id.as_opaque_key(), "gpu-a");
        assert_eq!(gpu.name, "RTX Test");
        assert_eq!((gpu.used_bytes, gpu.total_bytes), (4, 8));
        assert_eq!(
            decoded.history.networks[&decoded.snapshot.networks.value().unwrap()[0].id].down,
            VecDeque::from([7.0, 12.0])
        );
        assert_eq!(decoded.history.cpu, VecDeque::from([11.0]));
        assert_eq!(
            decoded.history.gpu_memory_used[&GpuId::from_opaque_key("gpu-a")],
            VecDeque::from([2.0, 4.0])
        );
        assert_eq!(decoded.history_capacity, 120);
        assert!(!decoded.snapshot.process_disk_io.is_observable());
        assert!(!decoded.snapshot.process_network_io.is_observable());
    }

    #[test]
    fn transport_round_trip_preserves_collection_outcomes() {
        let mut state = MonitorState::default();
        state.snapshot.process_disk_io = Collection::available(Vec::new());
        state.snapshot.process_network_io =
            Collection::unavailable(CollectionUnavailable::PermissionDenied);
        state.snapshot.networks = Collection::degraded(Vec::new());

        let json = serde_json::to_string(&WebMonitorState::from(&state)).unwrap();
        let decoded = MonitorState::from(serde_json::from_str::<WebMonitorState>(&json).unwrap());

        assert_eq!(decoded.snapshot.process_disk_io.value(), Some(&Vec::new()));
        assert_eq!(
            decoded.snapshot.process_network_io.status(),
            crate::core::model::CollectionStatus::Unavailable(
                CollectionUnavailable::PermissionDenied
            )
        );
        assert_eq!(
            decoded.snapshot.networks.status(),
            crate::core::model::CollectionStatus::Degraded
        );
    }
}
