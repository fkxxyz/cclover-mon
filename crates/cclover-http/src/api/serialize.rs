use super::model::*;

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
