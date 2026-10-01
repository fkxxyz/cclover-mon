use cclover_core::model::{
    DiskSnapshot, FanSnapshot, GpuSnapshot, MemorySnapshot, MonitorHistory, MonitorState,
    NetworkDirectionHistory, NetworkSnapshot, ProcessCpuUsage, ProcessDiskIo, ProcessMemoryUsage,
    ProcessNetworkIo, SystemSnapshot, TemperatureSnapshot,
};

use super::model::*;

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
