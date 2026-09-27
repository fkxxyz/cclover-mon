use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};

use crate::core::model::{
    DiskId, DiskSnapshot, MemorySnapshot, MonitorHistory, MonitorState, NetworkDirectionHistory,
    NetworkId, NetworkSnapshot, ProcessCpuUsage, ProcessMemoryUsage, SystemSnapshot,
    TemperatureSnapshot,
};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct WebMonitorState {
    snapshot: WebSystemSnapshot,
    history: WebMonitorHistory,
    history_capacity: usize,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct WebSystemSnapshot {
    cpu_percent: Option<f64>,
    memory: Option<WebMemorySnapshot>,
    top_cpu: Vec<WebProcessCpu>,
    top_memory: Vec<WebProcessMemory>,
    networks: Vec<WebNetworkSnapshot>,
    disks: Vec<WebDiskSnapshot>,
    temperatures: Vec<WebTemperatureSnapshot>,
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
struct WebTemperatureSnapshot {
    id: String,
    name: String,
    celsius: f64,
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
    networks: BTreeMap<String, WebDirectionHistory>,
    disks: BTreeMap<String, VecDeque<f64>>,
    temperatures: BTreeMap<String, VecDeque<f64>>,
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
            cpu_percent: snapshot.cpu_percent,
            memory: snapshot.memory.as_ref().map(WebMemorySnapshot::from),
            top_cpu: snapshot.top_cpu.iter().map(WebProcessCpu::from).collect(),
            top_memory: snapshot
                .top_memory
                .iter()
                .map(WebProcessMemory::from)
                .collect(),
            networks: snapshot
                .networks
                .iter()
                .map(WebNetworkSnapshot::from)
                .collect(),
            disks: snapshot.disks.iter().map(WebDiskSnapshot::from).collect(),
            temperatures: snapshot
                .temperatures
                .iter()
                .map(WebTemperatureSnapshot::from)
                .collect(),
        }
    }
}

impl From<WebSystemSnapshot> for SystemSnapshot {
    fn from(snapshot: WebSystemSnapshot) -> Self {
        Self {
            cpu_percent: snapshot.cpu_percent,
            memory: snapshot.memory.map(MemorySnapshot::from),
            top_cpu: snapshot
                .top_cpu
                .into_iter()
                .map(ProcessCpuUsage::from)
                .collect(),
            top_memory: snapshot
                .top_memory
                .into_iter()
                .map(ProcessMemoryUsage::from)
                .collect(),
            networks: snapshot
                .networks
                .into_iter()
                .map(NetworkSnapshot::from)
                .collect(),
            disks: snapshot.disks.into_iter().map(DiskSnapshot::from).collect(),
            process_disk_io: None,
            process_network_io: None,
            temperatures: snapshot
                .temperatures
                .into_iter()
                .map(TemperatureSnapshot::from)
                .collect(),
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
    use crate::core::model::{ProcessDiskIo, ProcessInstanceId, ProcessNetworkIo};

    #[test]
    fn transport_excludes_core_only_process_attribution() {
        let mut state = MonitorState::default();
        let process = ProcessInstanceId {
            pid: 42,
            birth_marker: 7,
        };
        state.snapshot.process_disk_io = Some(vec![ProcessDiskIo {
            process,
            device: "nvme0n1".to_owned(),
            read_bytes_per_sec: 1.0,
            write_bytes_per_sec: 2.0,
        }]);
        state.snapshot.process_network_io = Some(vec![ProcessNetworkIo {
            process,
            interface: "eth0".to_owned(),
            rx_bytes_per_sec: 3.0,
            tx_bytes_per_sec: 4.0,
        }]);

        let json = serde_json::to_string(&WebMonitorState::from(&state)).unwrap();

        assert!(!json.contains("process_disk_io"));
        assert!(!json.contains("process_network_io"));
        assert!(!json.contains("nvme0n1"));
        assert!(!json.contains("eth0"));
    }

    #[test]
    fn transport_round_trip_preserves_panel_state() {
        let mut state = MonitorState::default();
        let network_id = NetworkId::from_opaque_key("network-a");
        state.snapshot.cpu_percent = Some(37.5);
        state.snapshot.memory = Some(MemorySnapshot {
            used_bytes: 10,
            total_bytes: 20,
            swap_used_bytes: 3,
            swap_total_bytes: 4,
        });
        state.snapshot.networks.push(NetworkSnapshot {
            id: network_id.clone(),
            name: "eth0".to_owned(),
            down_bytes_per_sec: 12.0,
            up_bytes_per_sec: 5.0,
        });
        state.history.cpu.push_back(11.0);
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

        assert_eq!(decoded.snapshot.cpu_percent, Some(37.5));
        assert_eq!(decoded.snapshot.memory.unwrap().used_bytes, 10);
        assert_eq!(decoded.snapshot.networks[0].name, "eth0");
        assert_eq!(
            decoded.history.networks[&decoded.snapshot.networks[0].id].down,
            VecDeque::from([7.0, 12.0])
        );
        assert_eq!(decoded.history.cpu, VecDeque::from([11.0]));
        assert_eq!(decoded.history_capacity, 120);
        assert!(decoded.snapshot.process_disk_io.is_none());
        assert!(decoded.snapshot.process_network_io.is_none());
    }
}
