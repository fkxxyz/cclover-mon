use super::*;

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

impl ApiV1Collection<Vec<ApiV1GpuSnapshot>> {
    pub(super) fn project_legacy_gpu_memory(
        &self,
    ) -> ApiV1Collection<Vec<ApiV1LegacyGpuMemorySnapshot>> {
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

#[derive(Clone, Debug, Serialize)]
pub(super) struct ApiV1ProcessCpu {
    pub(super) name: String,
    pub(super) percent: f64,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct ApiV1ProcessMemory {
    pub(super) name: String,
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
    pub(super) pid: u32,
    pub(super) birth_marker: u64,
    pub(super) name: Option<String>,
    pub(super) disk_id: String,
    pub(super) device: String,
    pub(super) read_bytes_per_sec: f64,
    pub(super) write_bytes_per_sec: f64,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct ApiV1ProcessNetworkIo {
    pub(super) pid: u32,
    pub(super) birth_marker: u64,
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

#[derive(Clone, Debug, Serialize)]
pub(super) struct ApiV1LegacyGpuMemorySnapshot {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) used_bytes: u64,
    pub(super) total_bytes: u64,
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
