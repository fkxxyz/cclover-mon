use super::schema::*;
use serde::Serialize;
use std::collections::{BTreeMap, VecDeque};

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
pub(super) struct ApiV1LegacyGpuMemory {
    pub(super) gpu_memory: ApiV1Collection<Vec<ApiV1LegacyGpuMemorySnapshot>>,
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
pub(super) struct ApiV1LegacyGpuMemoryHistory<'a> {
    pub(super) history_capacity: usize,
    pub(super) gpu_memory_used: &'a BTreeMap<String, VecDeque<f64>>,
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
