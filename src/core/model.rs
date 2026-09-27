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
pub struct ProcessCpu {
    pub name: String,
    pub percent: f64,
}

#[derive(Clone, Debug)]
pub struct ProcessMemory {
    pub name: String,
    pub bytes: u64,
}

#[derive(Clone, Debug)]
pub struct NetworkSnapshot {
    pub name: String,
    pub down_bytes_per_sec: f64,
    pub up_bytes_per_sec: f64,
}

#[derive(Clone, Debug)]
pub struct DiskSnapshot {
    pub name: String,
    pub bytes_per_sec: f64,
}

#[derive(Clone, Debug)]
pub struct TemperatureSnapshot {
    pub name: String,
    pub celsius: f64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessDiskIoCounter {
    pub pid: u32,
    pub device: String,
    pub read_bytes: u64,
    pub write_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessNetworkIoCounter {
    pub pid: u32,
    pub interface: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProcessDiskIo {
    pub pid: u32,
    pub device: String,
    pub read_bytes_per_sec: f64,
    pub write_bytes_per_sec: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProcessNetworkIo {
    pub pid: u32,
    pub interface: String,
    pub rx_bytes_per_sec: f64,
    pub tx_bytes_per_sec: f64,
}

#[derive(Clone, Debug, Default)]
pub struct SystemSnapshot {
    pub cpu_percent: Option<f64>,
    pub memory: Option<MemorySnapshot>,
    pub top_cpu: Vec<ProcessCpu>,
    pub top_memory: Vec<ProcessMemory>,
    pub networks: Vec<NetworkSnapshot>,
    pub disks: Vec<DiskSnapshot>,
    pub process_disk_io: Option<Vec<ProcessDiskIo>>,
    pub process_network_io: Option<Vec<ProcessNetworkIo>>,
    pub temperatures: Vec<TemperatureSnapshot>,
}

#[derive(Clone, Debug, Default)]
pub struct DirectionHistory {
    pub down: VecDeque<f64>,
    pub up: VecDeque<f64>,
}

#[derive(Clone, Debug, Default)]
pub struct MonitorHistory {
    pub cpu: VecDeque<f64>,
    pub memory_used: VecDeque<f64>,
    pub swap_used: VecDeque<f64>,
    pub networks: BTreeMap<String, DirectionHistory>,
    pub disks: BTreeMap<String, VecDeque<f64>>,
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
    pub total_jiffies: u64,
    pub idle_jiffies: u64,
    pub logical_cpu_count: usize,
}

#[derive(Clone, Debug)]
pub struct ProcessCounter {
    pub pid: u32,
    pub name: String,
    pub cpu_ticks: u64,
    pub rss_bytes: u64,
}

#[derive(Clone, Debug)]
pub struct NetworkCounter {
    pub name: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

#[derive(Clone, Debug)]
pub struct DiskCounter {
    pub name: String,
    pub read_bytes: u64,
    pub write_bytes: u64,
}

#[derive(Clone, Debug)]
pub struct RawSnapshot {
    pub collected_at: Instant,
    pub cpu: Option<CpuCounter>,
    pub memory: Option<MemorySnapshot>,
    pub processes: Vec<ProcessCounter>,
    pub networks: Vec<NetworkCounter>,
    pub disks: Vec<DiskCounter>,
    pub process_disk_io: Option<Vec<ProcessDiskIoCounter>>,
    pub process_network_io: Option<Vec<ProcessNetworkIoCounter>>,
    pub temperatures: Vec<TemperatureSnapshot>,
}

impl Default for RawSnapshot {
    fn default() -> Self {
        Self {
            collected_at: Instant::now(),
            cpu: None,
            memory: None,
            processes: Vec::new(),
            networks: Vec::new(),
            disks: Vec::new(),
            process_disk_io: None,
            process_network_io: None,
            temperatures: Vec::new(),
        }
    }
}
