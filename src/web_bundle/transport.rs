use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};

use cclover_core::model::{
    Collection, CollectionUnavailable, DiskId, DiskMetadata, DiskSnapshot, FanId, FanSnapshot,
    GpuId, GpuSnapshot, MemorySnapshot, MonitorHistory, MonitorState, NetworkDirectionHistory,
    NetworkId, NetworkSnapshot, ProcessCpuUsage, ProcessDiskIo, ProcessInstanceId,
    ProcessMemoryUsage, ProcessNetworkIo, SystemSnapshot, TemperatureId, TemperatureSnapshot,
};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct WebMonitorState {
    snapshot: WebSystemSnapshot,
    history: WebMonitorHistory,
    history_capacity: usize,
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
    fans: WebCollection<Vec<WebFanSnapshot>>,
    gpus: WebCollection<Vec<WebGpuSnapshot>>,
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
    system_label: String,
    associated_labels: Vec<String>,
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
struct WebFanSnapshot {
    id: String,
    name: String,
    rpm: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct WebGpuSnapshot {
    id: String,
    name: String,
    utilization_percent: Option<f64>,
    memory_used_bytes: Option<u64>,
    memory_total_bytes: Option<u64>,
    temperature_celsius: Option<f64>,
    power_watts: Option<f64>,
    core_clock_mhz: Option<u64>,
    fan_percent: Option<f64>,
    fan_rpm: Option<u64>,
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
    gpu_utilization: BTreeMap<String, VecDeque<f64>>,
    gpu_memory_used: BTreeMap<String, VecDeque<f64>>,
    gpu_temperature: BTreeMap<String, VecDeque<f64>>,
    networks: BTreeMap<String, WebDirectionHistory>,
    disks: BTreeMap<String, VecDeque<f64>>,
    temperatures: BTreeMap<String, VecDeque<f64>>,
    fans: BTreeMap<String, VecDeque<f64>>,
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
            fans: WebCollection::from_core(&snapshot.fans, |rows| {
                rows.iter().map(WebFanSnapshot::from).collect()
            }),
            gpus: WebCollection::from_core(&snapshot.gpus, |rows| {
                rows.iter().map(WebGpuSnapshot::from).collect()
            }),
        }
    }
}

impl From<WebSystemSnapshot> for SystemSnapshot {
    fn from(snapshot: WebSystemSnapshot) -> Self {
        Self {
            cpu_percent: snapshot.cpu_percent.into_core(|value| value),
            memory: snapshot.memory.into_core(MemorySnapshot::from),
            processes: Default::default(),
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
            fans: snapshot
                .fans
                .into_core(|rows| rows.into_iter().map(FanSnapshot::from).collect()),
            gpus: snapshot
                .gpus
                .into_core(|rows| rows.into_iter().map(GpuSnapshot::from).collect()),
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
            system_label: value.metadata.system_label.clone(),
            associated_labels: value.metadata.associated_labels.clone(),
            bytes_per_sec: value.bytes_per_sec,
        }
    }
}

impl From<WebDiskSnapshot> for DiskSnapshot {
    fn from(value: WebDiskSnapshot) -> Self {
        Self {
            id: DiskId::from_opaque_key(value.id),
            metadata: DiskMetadata {
                system_label: value.system_label,
                associated_labels: value.associated_labels,
            },
            bytes_per_sec: value.bytes_per_sec,
        }
    }
}

impl From<&ProcessDiskIo> for WebProcessDiskIo {
    fn from(value: &ProcessDiskIo) -> Self {
        Self {
            pid: value.process.pid,
            birth_marker: value.process.birth_marker,
            name: value.name.as_deref().map(str::to_owned),
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
            name: value.name.map(Into::into),
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
            name: value.name.as_deref().map(str::to_owned),
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
            name: value.name.map(Into::into),
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
            id: value.id.as_opaque_key().to_owned(),
            name: value.name.clone(),
            celsius: value.celsius,
        }
    }
}

impl From<WebTemperatureSnapshot> for TemperatureSnapshot {
    fn from(value: WebTemperatureSnapshot) -> Self {
        Self {
            id: TemperatureId::from_opaque_key(value.id),
            name: value.name,
            celsius: value.celsius,
        }
    }
}

impl From<&FanSnapshot> for WebFanSnapshot {
    fn from(value: &FanSnapshot) -> Self {
        Self {
            id: value.id.as_opaque_key().to_owned(),
            name: value.name.clone(),
            rpm: value.rpm,
        }
    }
}

impl From<WebFanSnapshot> for FanSnapshot {
    fn from(value: WebFanSnapshot) -> Self {
        Self {
            id: FanId::from_opaque_key(value.id),
            name: value.name,
            rpm: value.rpm,
        }
    }
}

impl From<&GpuSnapshot> for WebGpuSnapshot {
    fn from(value: &GpuSnapshot) -> Self {
        Self {
            id: value.id.as_opaque_key().to_owned(),
            name: value.name.clone(),
            utilization_percent: value.utilization_percent,
            memory_used_bytes: value.memory_used_bytes,
            memory_total_bytes: value.memory_total_bytes,
            temperature_celsius: value.temperature_celsius,
            power_watts: value.power_watts,
            core_clock_mhz: value.core_clock_mhz,
            fan_percent: value.fan_percent,
            fan_rpm: value.fan_rpm,
        }
    }
}

impl From<WebGpuSnapshot> for GpuSnapshot {
    fn from(value: WebGpuSnapshot) -> Self {
        Self {
            id: GpuId::from_opaque_key(value.id),
            name: value.name,
            utilization_percent: value.utilization_percent,
            memory_used_bytes: value.memory_used_bytes,
            memory_total_bytes: value.memory_total_bytes,
            temperature_celsius: value.temperature_celsius,
            power_watts: value.power_watts,
            core_clock_mhz: value.core_clock_mhz,
            fan_percent: value.fan_percent,
            fan_rpm: value.fan_rpm,
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
            gpu_utilization: history
                .gpu_utilization
                .iter()
                .map(|(id, values)| (id.as_opaque_key().to_owned(), values.clone()))
                .collect(),
            gpu_memory_used: history
                .gpu_memory_used
                .iter()
                .map(|(id, values)| (id.as_opaque_key().to_owned(), values.clone()))
                .collect(),
            gpu_temperature: history
                .gpu_temperature
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
            temperatures: history
                .temperatures
                .iter()
                .map(|(id, values)| (id.as_opaque_key().to_owned(), values.clone()))
                .collect(),
            fans: history
                .fans
                .iter()
                .map(|(id, values)| (id.as_opaque_key().to_owned(), values.clone()))
                .collect(),
        }
    }
}

impl From<WebMonitorHistory> for MonitorHistory {
    fn from(history: WebMonitorHistory) -> Self {
        Self {
            cpu: history.cpu,
            memory_used: history.memory_used,
            swap_used: history.swap_used,
            gpu_utilization: history
                .gpu_utilization
                .into_iter()
                .map(|(id, values)| (GpuId::from_opaque_key(id), values))
                .collect(),
            gpu_memory_used: history
                .gpu_memory_used
                .into_iter()
                .map(|(id, values)| (GpuId::from_opaque_key(id), values))
                .collect(),
            gpu_temperature: history
                .gpu_temperature
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
            temperatures: history
                .temperatures
                .into_iter()
                .map(|(id, values)| (TemperatureId::from_opaque_key(id), values))
                .collect(),
            fans: history
                .fans
                .into_iter()
                .map(|(id, values)| (FanId::from_opaque_key(id), values))
                .collect(),
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
            name: Some("worker".into()),
            disk_id: DiskId::from_opaque_key("disk-a"),
            device: "nvme0n1".to_owned(),
            read_bytes_per_sec: 1.0,
            write_bytes_per_sec: 2.0,
        }]);
        state.snapshot.process_network_io = Collection::available(vec![ProcessNetworkIo {
            process,
            name: Some("worker".into()),
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
        state.snapshot.disks = Collection::available(vec![DiskSnapshot {
            id: DiskId::from_opaque_key("disk-a"),
            metadata: DiskMetadata {
                system_label: "Disk 0".to_owned(),
                associated_labels: vec!["C:".to_owned(), "D:".to_owned()],
            },
            bytes_per_sec: 9.0,
        }]);
        state.snapshot.gpus = Collection::available(vec![GpuSnapshot {
            id: GpuId::from_opaque_key("gpu-a"),
            name: "RTX Test".to_owned(),
            utilization_percent: Some(42.0),
            memory_used_bytes: Some(4),
            memory_total_bytes: Some(8),
            temperature_celsius: Some(63.0),
            power_watts: Some(145.0),
            core_clock_mhz: Some(1830),
            fan_percent: Some(37.0),
            fan_rpm: None,
        }]);
        state.history.cpu.push_back(11.0);
        state.history.gpu_utilization.insert(
            GpuId::from_opaque_key("gpu-a"),
            VecDeque::from([40.0, 42.0]),
        );
        state
            .history
            .gpu_memory_used
            .insert(GpuId::from_opaque_key("gpu-a"), VecDeque::from([2.0, 4.0]));
        state.history.gpu_temperature.insert(
            GpuId::from_opaque_key("gpu-a"),
            VecDeque::from([61.0, 63.0]),
        );
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
        let disk = &decoded.snapshot.disks.value().unwrap()[0];
        assert_eq!(disk.metadata.system_label, "Disk 0");
        assert_eq!(disk.metadata.associated_labels, ["C:", "D:"]);
        let gpu = &decoded.snapshot.gpus.value().unwrap()[0];
        assert_eq!(gpu.id.as_opaque_key(), "gpu-a");
        assert_eq!(gpu.name, "RTX Test");
        assert_eq!(gpu.utilization_percent, Some(42.0));
        assert_eq!(
            (gpu.memory_used_bytes, gpu.memory_total_bytes),
            (Some(4), Some(8))
        );
        assert_eq!(gpu.temperature_celsius, Some(63.0));
        assert_eq!(gpu.power_watts, Some(145.0));
        assert_eq!(gpu.core_clock_mhz, Some(1830));
        assert_eq!(gpu.fan_percent, Some(37.0));
        assert_eq!(
            decoded.history.networks[&decoded.snapshot.networks.value().unwrap()[0].id].down,
            VecDeque::from([7.0, 12.0])
        );
        assert_eq!(decoded.history.cpu, VecDeque::from([11.0]));
        assert_eq!(
            decoded.history.gpu_utilization[&GpuId::from_opaque_key("gpu-a")],
            VecDeque::from([40.0, 42.0])
        );
        assert_eq!(
            decoded.history.gpu_memory_used[&GpuId::from_opaque_key("gpu-a")],
            VecDeque::from([2.0, 4.0])
        );
        assert_eq!(
            decoded.history.gpu_temperature[&GpuId::from_opaque_key("gpu-a")],
            VecDeque::from([61.0, 63.0])
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
            cclover_core::model::CollectionStatus::Unavailable(
                CollectionUnavailable::PermissionDenied
            )
        );
        assert_eq!(
            decoded.snapshot.networks.status(),
            cclover_core::model::CollectionStatus::Degraded
        );
    }
}
