use std::collections::{BTreeMap, VecDeque};

use serde::Serialize;

use crate::core::model::{
    Collection, CollectionUnavailable, DiskSnapshot, FanSnapshot, GpuSnapshot, MemorySnapshot,
    MonitorHistory, MonitorState, NetworkDirectionHistory, NetworkSnapshot, ProcessCpuUsage,
    ProcessDiskIo, ProcessMemoryUsage, ProcessNetworkIo, SystemSnapshot, TemperatureSnapshot,
};

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct ApiV1State {
    snapshot: ApiV1Snapshot,
    history: ApiV1History,
    history_capacity: usize,
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
    GpuMemoryLegacy,
    Processes,
    HistoryCpu,
    HistoryMemory,
    HistoryGpus,
    HistoryGpuMemoryLegacy,
    HistoryDisks,
    HistoryNetworks,
    HistoryTemperatures,
    HistoryFans,
}

impl ApiV1State {
    pub(crate) fn serialize_slice(&self, slice: ApiV1Slice) -> serde_json::Result<String> {
        match slice {
            ApiV1Slice::Cpu => serde_json::to_string(&ApiV1Cpu {
                cpu_percent: &self.snapshot.cpu_percent,
                top_cpu: &self.snapshot.top_cpu,
            }),
            ApiV1Slice::Memory => serde_json::to_string(&ApiV1Memory {
                memory: &self.snapshot.memory,
                top_memory: &self.snapshot.top_memory,
            }),
            ApiV1Slice::Disks => serde_json::to_string(&ApiV1Disks {
                disks: &self.snapshot.disks,
                process_disk_io: &self.snapshot.process_disk_io,
            }),
            ApiV1Slice::Networks => serde_json::to_string(&ApiV1Networks {
                networks: &self.snapshot.networks,
                process_network_io: &self.snapshot.process_network_io,
            }),
            ApiV1Slice::Temperatures => serde_json::to_string(&ApiV1Temperatures {
                temperatures: &self.snapshot.temperatures,
            }),
            ApiV1Slice::Fans => serde_json::to_string(&ApiV1Fans {
                fans: &self.snapshot.fans,
            }),
            ApiV1Slice::Gpus => serde_json::to_string(&ApiV1Gpus {
                gpus: &self.snapshot.gpus,
            }),
            ApiV1Slice::GpuMemoryLegacy => serde_json::to_string(&ApiV1LegacyGpuMemory {
                gpu_memory: self.snapshot.gpus.project_legacy_gpu_memory(),
            }),
            ApiV1Slice::Processes => serde_json::to_string(&ApiV1Processes {
                top_cpu: &self.snapshot.top_cpu,
                top_memory: &self.snapshot.top_memory,
                disk_io: &self.snapshot.process_disk_io,
                network_io: &self.snapshot.process_network_io,
            }),
            ApiV1Slice::HistoryCpu => serde_json::to_string(&ApiV1CpuHistory {
                history_capacity: self.history_capacity,
                cpu: &self.history.cpu,
            }),
            ApiV1Slice::HistoryMemory => serde_json::to_string(&ApiV1MemoryHistory {
                history_capacity: self.history_capacity,
                memory_used: &self.history.memory_used,
                swap_used: &self.history.swap_used,
            }),
            ApiV1Slice::HistoryGpus => serde_json::to_string(&ApiV1GpuHistory {
                history_capacity: self.history_capacity,
                utilization: &self.history.gpu_utilization,
                memory_used: &self.history.gpu_memory_used,
                temperature: &self.history.gpu_temperature,
            }),
            ApiV1Slice::HistoryGpuMemoryLegacy => {
                serde_json::to_string(&ApiV1LegacyGpuMemoryHistory {
                    history_capacity: self.history_capacity,
                    gpu_memory_used: &self.history.gpu_memory_used,
                })
            }
            ApiV1Slice::HistoryDisks => serde_json::to_string(&ApiV1DisksHistory {
                history_capacity: self.history_capacity,
                disks: &self.history.disks,
            }),
            ApiV1Slice::HistoryNetworks => serde_json::to_string(&ApiV1NetworksHistory {
                history_capacity: self.history_capacity,
                networks: &self.history.networks,
            }),
            ApiV1Slice::HistoryTemperatures => serde_json::to_string(&ApiV1TemperaturesHistory {
                history_capacity: self.history_capacity,
                temperatures: &self.history.temperatures,
            }),
            ApiV1Slice::HistoryFans => serde_json::to_string(&ApiV1FansHistory {
                history_capacity: self.history_capacity,
                fans: &self.history.fans,
            }),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
struct ApiV1Snapshot {
    cpu_percent: ApiV1Collection<f64>,
    memory: ApiV1Collection<ApiV1MemorySnapshot>,
    top_cpu: ApiV1Collection<Vec<ApiV1ProcessCpu>>,
    top_memory: ApiV1Collection<Vec<ApiV1ProcessMemory>>,
    networks: ApiV1Collection<Vec<ApiV1NetworkSnapshot>>,
    disks: ApiV1Collection<Vec<ApiV1DiskSnapshot>>,
    process_disk_io: ApiV1Collection<Vec<ApiV1ProcessDiskIo>>,
    process_network_io: ApiV1Collection<Vec<ApiV1ProcessNetworkIo>>,
    temperatures: ApiV1Collection<Vec<ApiV1TemperatureSnapshot>>,
    fans: ApiV1Collection<Vec<ApiV1FanSnapshot>>,
    gpus: ApiV1Collection<Vec<ApiV1GpuSnapshot>>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "status", content = "value", rename_all = "snake_case")]
enum ApiV1Collection<T> {
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
    fn from_core<S>(source: &Collection<S>, map: impl FnOnce(&S) -> T) -> Self {
        match source {
            Collection::Available(value) => Self::Available(map(value)),
            Collection::Degraded(value) => Self::Degraded(map(value)),
            Collection::Unavailable(reason) => Self::Unavailable((*reason).into()),
        }
    }
}

impl ApiV1Collection<Vec<ApiV1GpuSnapshot>> {
    fn project_legacy_gpu_memory(&self) -> ApiV1Collection<Vec<ApiV1LegacyGpuMemorySnapshot>> {
        let project = |rows: &[ApiV1GpuSnapshot]| {
            rows.iter()
                .filter_map(|gpu| {
                    Some(ApiV1LegacyGpuMemorySnapshot {
                        id: gpu.id.clone(),
                        name: gpu.name.clone(),
                        used_bytes: gpu.memory_used_bytes?,
                        total_bytes: gpu.memory_total_bytes?,
                    })
                })
                .collect()
        };
        match self {
            Self::Available(rows) => ApiV1Collection::Available(project(rows)),
            Self::Degraded(rows) => ApiV1Collection::Degraded(project(rows)),
            Self::Unavailable(reason) => ApiV1Collection::Unavailable(*reason),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum ApiV1Unavailable {
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
struct ApiV1MemorySnapshot {
    used_bytes: u64,
    total_bytes: u64,
    swap_used_bytes: u64,
    swap_total_bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
struct ApiV1ProcessCpu {
    name: String,
    percent: f64,
}

#[derive(Clone, Debug, Serialize)]
struct ApiV1ProcessMemory {
    name: String,
    bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
struct ApiV1NetworkSnapshot {
    id: String,
    name: String,
    down_bytes_per_sec: f64,
    up_bytes_per_sec: f64,
}

#[derive(Clone, Debug, Serialize)]
struct ApiV1DiskSnapshot {
    id: String,
    name: String,
    bytes_per_sec: f64,
}

#[derive(Clone, Debug, Serialize)]
struct ApiV1ProcessDiskIo {
    pid: u32,
    birth_marker: u64,
    name: Option<String>,
    disk_id: String,
    device: String,
    read_bytes_per_sec: f64,
    write_bytes_per_sec: f64,
}

#[derive(Clone, Debug, Serialize)]
struct ApiV1ProcessNetworkIo {
    pid: u32,
    birth_marker: u64,
    name: Option<String>,
    network_id: String,
    interface: String,
    rx_bytes_per_sec: f64,
    tx_bytes_per_sec: f64,
}

#[derive(Clone, Debug, Serialize)]
struct ApiV1TemperatureSnapshot {
    id: String,
    name: String,
    celsius: f64,
}

#[derive(Clone, Debug, Serialize)]
struct ApiV1FanSnapshot {
    id: String,
    name: String,
    rpm: u64,
}

#[derive(Clone, Debug, Serialize)]
struct ApiV1GpuSnapshot {
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

#[derive(Clone, Debug, Serialize)]
struct ApiV1LegacyGpuMemorySnapshot {
    id: String,
    name: String,
    used_bytes: u64,
    total_bytes: u64,
}

#[derive(Clone, Debug, Default, Serialize)]
struct ApiV1DirectionHistory {
    down: VecDeque<f64>,
    up: VecDeque<f64>,
}

#[derive(Clone, Debug, Default, Serialize)]
struct ApiV1History {
    cpu: VecDeque<f64>,
    memory_used: VecDeque<f64>,
    swap_used: VecDeque<f64>,
    gpu_utilization: BTreeMap<String, VecDeque<f64>>,
    gpu_memory_used: BTreeMap<String, VecDeque<f64>>,
    gpu_temperature: BTreeMap<String, VecDeque<f64>>,
    networks: BTreeMap<String, ApiV1DirectionHistory>,
    disks: BTreeMap<String, VecDeque<f64>>,
    temperatures: BTreeMap<String, VecDeque<f64>>,
    fans: BTreeMap<String, VecDeque<f64>>,
}

#[derive(Serialize)]
struct ApiV1Cpu<'a> {
    cpu_percent: &'a ApiV1Collection<f64>,
    top_cpu: &'a ApiV1Collection<Vec<ApiV1ProcessCpu>>,
}

#[derive(Serialize)]
struct ApiV1Memory<'a> {
    memory: &'a ApiV1Collection<ApiV1MemorySnapshot>,
    top_memory: &'a ApiV1Collection<Vec<ApiV1ProcessMemory>>,
}

#[derive(Serialize)]
struct ApiV1Disks<'a> {
    disks: &'a ApiV1Collection<Vec<ApiV1DiskSnapshot>>,
    process_disk_io: &'a ApiV1Collection<Vec<ApiV1ProcessDiskIo>>,
}

#[derive(Serialize)]
struct ApiV1Networks<'a> {
    networks: &'a ApiV1Collection<Vec<ApiV1NetworkSnapshot>>,
    process_network_io: &'a ApiV1Collection<Vec<ApiV1ProcessNetworkIo>>,
}

#[derive(Serialize)]
struct ApiV1Temperatures<'a> {
    temperatures: &'a ApiV1Collection<Vec<ApiV1TemperatureSnapshot>>,
}

#[derive(Serialize)]
struct ApiV1Fans<'a> {
    fans: &'a ApiV1Collection<Vec<ApiV1FanSnapshot>>,
}

#[derive(Serialize)]
struct ApiV1Gpus<'a> {
    gpus: &'a ApiV1Collection<Vec<ApiV1GpuSnapshot>>,
}

#[derive(Serialize)]
struct ApiV1LegacyGpuMemory {
    gpu_memory: ApiV1Collection<Vec<ApiV1LegacyGpuMemorySnapshot>>,
}

#[derive(Serialize)]
struct ApiV1Processes<'a> {
    top_cpu: &'a ApiV1Collection<Vec<ApiV1ProcessCpu>>,
    top_memory: &'a ApiV1Collection<Vec<ApiV1ProcessMemory>>,
    disk_io: &'a ApiV1Collection<Vec<ApiV1ProcessDiskIo>>,
    network_io: &'a ApiV1Collection<Vec<ApiV1ProcessNetworkIo>>,
}

#[derive(Serialize)]
struct ApiV1CpuHistory<'a> {
    history_capacity: usize,
    cpu: &'a VecDeque<f64>,
}

#[derive(Serialize)]
struct ApiV1MemoryHistory<'a> {
    history_capacity: usize,
    memory_used: &'a VecDeque<f64>,
    swap_used: &'a VecDeque<f64>,
}

#[derive(Serialize)]
struct ApiV1GpuHistory<'a> {
    history_capacity: usize,
    utilization: &'a BTreeMap<String, VecDeque<f64>>,
    memory_used: &'a BTreeMap<String, VecDeque<f64>>,
    temperature: &'a BTreeMap<String, VecDeque<f64>>,
}

#[derive(Serialize)]
struct ApiV1LegacyGpuMemoryHistory<'a> {
    history_capacity: usize,
    gpu_memory_used: &'a BTreeMap<String, VecDeque<f64>>,
}

#[derive(Serialize)]
struct ApiV1DisksHistory<'a> {
    history_capacity: usize,
    disks: &'a BTreeMap<String, VecDeque<f64>>,
}

#[derive(Serialize)]
struct ApiV1NetworksHistory<'a> {
    history_capacity: usize,
    networks: &'a BTreeMap<String, ApiV1DirectionHistory>,
}

#[derive(Serialize)]
struct ApiV1TemperaturesHistory<'a> {
    history_capacity: usize,
    temperatures: &'a BTreeMap<String, VecDeque<f64>>,
}

#[derive(Serialize)]
struct ApiV1FansHistory<'a> {
    history_capacity: usize,
    fans: &'a BTreeMap<String, VecDeque<f64>>,
}

impl From<&MonitorState> for ApiV1State {
    fn from(state: &MonitorState) -> Self {
        Self {
            snapshot: ApiV1Snapshot::from(&state.snapshot),
            history: ApiV1History::from(&state.history),
            history_capacity: state.history_capacity,
        }
    }
}

impl From<&SystemSnapshot> for ApiV1Snapshot {
    fn from(snapshot: &SystemSnapshot) -> Self {
        Self {
            cpu_percent: ApiV1Collection::from_core(&snapshot.cpu_percent, |value| *value),
            memory: ApiV1Collection::from_core(&snapshot.memory, |value| {
                ApiV1MemorySnapshot::from(value)
            }),
            top_cpu: ApiV1Collection::from_core(&snapshot.top_cpu, |rows| {
                rows.iter().map(ApiV1ProcessCpu::from).collect()
            }),
            top_memory: ApiV1Collection::from_core(&snapshot.top_memory, |rows| {
                rows.iter().map(ApiV1ProcessMemory::from).collect()
            }),
            networks: ApiV1Collection::from_core(&snapshot.networks, |rows| {
                rows.iter().map(ApiV1NetworkSnapshot::from).collect()
            }),
            disks: ApiV1Collection::from_core(&snapshot.disks, |rows| {
                rows.iter().map(ApiV1DiskSnapshot::from).collect()
            }),
            process_disk_io: ApiV1Collection::from_core(&snapshot.process_disk_io, |rows| {
                rows.iter().map(ApiV1ProcessDiskIo::from).collect()
            }),
            process_network_io: ApiV1Collection::from_core(&snapshot.process_network_io, |rows| {
                rows.iter().map(ApiV1ProcessNetworkIo::from).collect()
            }),
            temperatures: ApiV1Collection::from_core(&snapshot.temperatures, |rows| {
                rows.iter().map(ApiV1TemperatureSnapshot::from).collect()
            }),
            fans: ApiV1Collection::from_core(&snapshot.fans, |rows| {
                rows.iter().map(ApiV1FanSnapshot::from).collect()
            }),
            gpus: ApiV1Collection::from_core(&snapshot.gpus, |rows| {
                rows.iter().map(ApiV1GpuSnapshot::from).collect()
            }),
        }
    }
}

impl From<&MemorySnapshot> for ApiV1MemorySnapshot {
    fn from(value: &MemorySnapshot) -> Self {
        Self {
            used_bytes: value.used_bytes,
            total_bytes: value.total_bytes,
            swap_used_bytes: value.swap_used_bytes,
            swap_total_bytes: value.swap_total_bytes,
        }
    }
}

impl From<&ProcessCpuUsage> for ApiV1ProcessCpu {
    fn from(value: &ProcessCpuUsage) -> Self {
        Self {
            name: value.name.clone(),
            percent: value.percent,
        }
    }
}

impl From<&ProcessMemoryUsage> for ApiV1ProcessMemory {
    fn from(value: &ProcessMemoryUsage) -> Self {
        Self {
            name: value.name.clone(),
            bytes: value.bytes,
        }
    }
}

impl From<&NetworkSnapshot> for ApiV1NetworkSnapshot {
    fn from(value: &NetworkSnapshot) -> Self {
        Self {
            id: value.id.as_opaque_key().to_owned(),
            name: value.name.clone(),
            down_bytes_per_sec: value.down_bytes_per_sec,
            up_bytes_per_sec: value.up_bytes_per_sec,
        }
    }
}

impl From<&DiskSnapshot> for ApiV1DiskSnapshot {
    fn from(value: &DiskSnapshot) -> Self {
        Self {
            id: value.id.as_opaque_key().to_owned(),
            name: value.metadata.system_label.clone(),
            bytes_per_sec: value.bytes_per_sec,
        }
    }
}

impl From<&ProcessDiskIo> for ApiV1ProcessDiskIo {
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

impl From<&ProcessNetworkIo> for ApiV1ProcessNetworkIo {
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

impl From<&TemperatureSnapshot> for ApiV1TemperatureSnapshot {
    fn from(value: &TemperatureSnapshot) -> Self {
        Self {
            id: value.id.as_opaque_key().to_owned(),
            name: value.name.clone(),
            celsius: value.celsius,
        }
    }
}

impl From<&FanSnapshot> for ApiV1FanSnapshot {
    fn from(value: &FanSnapshot) -> Self {
        Self {
            id: value.id.as_opaque_key().to_owned(),
            name: value.name.clone(),
            rpm: value.rpm,
        }
    }
}

impl From<&GpuSnapshot> for ApiV1GpuSnapshot {
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

impl From<&NetworkDirectionHistory> for ApiV1DirectionHistory {
    fn from(value: &NetworkDirectionHistory) -> Self {
        Self {
            down: value.down.clone(),
            up: value.up.clone(),
        }
    }
}

impl From<&MonitorHistory> for ApiV1History {
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
                        ApiV1DirectionHistory::from(value),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::model::{GpuId, ProcessInstanceId};
    use serde_json::json;

    fn representative_state() -> MonitorState {
        let mut state = MonitorState {
            history_capacity: 120,
            ..MonitorState::default()
        };
        state.snapshot.cpu_percent = Collection::available(37.5);
        state.snapshot.memory = Collection::available(MemorySnapshot {
            used_bytes: 10,
            total_bytes: 20,
            swap_used_bytes: 3,
            swap_total_bytes: 4,
        });
        state.snapshot.gpus = Collection::available(vec![
            GpuSnapshot {
                id: GpuId::from_opaque_key("gpu-a"),
                name: "GPU A".to_owned(),
                utilization_percent: Some(42.0),
                memory_used_bytes: Some(4),
                memory_total_bytes: Some(8),
                temperature_celsius: Some(63.0),
                power_watts: Some(145.0),
                core_clock_mhz: Some(1830),
                fan_percent: Some(37.0),
                fan_rpm: None,
            },
            GpuSnapshot {
                id: GpuId::from_opaque_key("gpu-without-memory"),
                name: "GPU B".to_owned(),
                utilization_percent: Some(5.0),
                memory_used_bytes: None,
                memory_total_bytes: None,
                temperature_celsius: None,
                power_watts: None,
                core_clock_mhz: None,
                fan_percent: None,
                fan_rpm: None,
            },
        ]);
        state.snapshot.process_disk_io = Collection::available(vec![ProcessDiskIo {
            process: ProcessInstanceId {
                pid: 42,
                birth_marker: 7,
            },
            name: Some("worker".into()),
            disk_id: crate::core::model::DiskId::from_opaque_key("disk-a"),
            device: "nvme0n1".to_owned(),
            read_bytes_per_sec: 1.0,
            write_bytes_per_sec: 2.0,
        }]);
        state.history.cpu = VecDeque::from([11.0, 12.0]);
        state
            .history
            .gpu_memory_used
            .insert(GpuId::from_opaque_key("gpu-a"), VecDeque::from([2.0, 4.0]));
        state
    }

    #[test]
    fn gpu_wire_shape_is_an_explicit_v1_contract() {
        let state = ApiV1State::from(&representative_state());
        let actual: serde_json::Value =
            serde_json::from_str(&state.serialize_slice(ApiV1Slice::Gpus).unwrap()).unwrap();

        assert_eq!(
            actual,
            json!({
                "gpus": {
                    "status": "available",
                    "value": [
                        {
                            "id": "gpu-a",
                            "name": "GPU A",
                            "utilization_percent": 42.0,
                            "memory_used_bytes": 4,
                            "memory_total_bytes": 8,
                            "temperature_celsius": 63.0,
                            "power_watts": 145.0,
                            "core_clock_mhz": 1830,
                            "fan_percent": 37.0,
                            "fan_rpm": null
                        },
                        {
                            "id": "gpu-without-memory",
                            "name": "GPU B",
                            "utilization_percent": 5.0,
                            "memory_used_bytes": null,
                            "memory_total_bytes": null,
                            "temperature_celsius": null,
                            "power_watts": null,
                            "core_clock_mhz": null,
                            "fan_percent": null,
                            "fan_rpm": null
                        }
                    ]
                }
            })
        );
    }

    #[test]
    fn legacy_gpu_memory_routes_keep_their_original_payload_shape() {
        let state = ApiV1State::from(&representative_state());
        let snapshot: serde_json::Value =
            serde_json::from_str(&state.serialize_slice(ApiV1Slice::GpuMemoryLegacy).unwrap())
                .unwrap();
        let history: serde_json::Value = serde_json::from_str(
            &state
                .serialize_slice(ApiV1Slice::HistoryGpuMemoryLegacy)
                .unwrap(),
        )
        .unwrap();

        assert_eq!(
            snapshot,
            json!({
                "gpu_memory": {
                    "status": "available",
                    "value": [{
                        "id": "gpu-a",
                        "name": "GPU A",
                        "used_bytes": 4,
                        "total_bytes": 8
                    }]
                }
            })
        );
        assert_eq!(
            history,
            json!({
                "history_capacity": 120,
                "gpu_memory_used": {"gpu-a": [2.0, 4.0]}
            })
        );
    }

    #[test]
    fn collection_status_and_unavailable_reason_are_part_of_v1() {
        let mut state = MonitorState::default();
        state.snapshot.cpu_percent =
            Collection::unavailable(CollectionUnavailable::PermissionDenied);
        state.snapshot.networks = Collection::degraded(Vec::new());
        let api = ApiV1State::from(&state);

        let cpu: serde_json::Value =
            serde_json::from_str(&api.serialize_slice(ApiV1Slice::Cpu).unwrap()).unwrap();
        let networks: serde_json::Value =
            serde_json::from_str(&api.serialize_slice(ApiV1Slice::Networks).unwrap()).unwrap();

        assert_eq!(
            cpu["cpu_percent"],
            json!({"status": "unavailable", "value": "permission_denied"})
        );
        assert_eq!(
            networks["networks"],
            json!({"status": "degraded", "value": []})
        );
    }

    #[test]
    fn full_state_shape_is_owned_by_api_v1() {
        let actual = serde_json::to_value(ApiV1State::from(&representative_state())).unwrap();

        assert_eq!(actual["history_capacity"], 120);
        assert_eq!(actual["snapshot"]["cpu_percent"]["status"], "available");
        assert_eq!(actual["snapshot"]["cpu_percent"]["value"], 37.5);
        assert_eq!(actual["snapshot"]["gpus"]["value"][0]["id"], "gpu-a");
        assert_eq!(actual["history"]["cpu"], json!([11.0, 12.0]));
        assert!(actual["snapshot"].get("gpu_memory").is_none());
    }
}
