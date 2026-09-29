use std::collections::VecDeque;

use cclover_core::model::{
    Collection, DiskSnapshot, GpuMemorySnapshot, MemorySnapshot, MonitorState,
    NetworkDirectionHistory, NetworkSnapshot, ProcessCpuUsage, ProcessDiskIo, ProcessMemoryUsage,
    ProcessNetworkIo, TemperatureSnapshot,
};

pub const TEMPERATURE_SECTION: &str = "TEMPERATURE";
pub const GPU_MEMORY_SECTION: &str = "GPU MEMORY";
pub const DISK_SECTION: &str = "DISK I/O";
pub const NETWORK_SECTION: &str = "NETWORK";

#[derive(Clone, Copy)]
pub struct Dashboard<'a> {
    state: &'a MonitorState,
}

impl<'a> Dashboard<'a> {
    pub const fn new(state: &'a MonitorState) -> Self {
        Self { state }
    }

    pub fn history_capacity(self) -> usize {
        self.state.history_capacity.max(1)
    }

    pub fn memory(self) -> MemoryPanel<'a> {
        MemoryPanel {
            memory: self.state.snapshot.memory.value(),
            processes: self
                .state
                .snapshot
                .top_memory
                .value()
                .map(Vec::as_slice)
                .unwrap_or_default(),
            history: &self.state.history.memory_used,
        }
    }

    pub fn cpu(self) -> CpuPanel<'a> {
        CpuPanel {
            percent: self.state.snapshot.cpu_percent.value().copied(),
            processes: self
                .state
                .snapshot
                .top_cpu
                .value()
                .map(Vec::as_slice)
                .unwrap_or_default(),
            history: &self.state.history.cpu,
        }
    }

    pub fn temperature_count(self) -> usize {
        self.state.snapshot.temperatures.value().map_or(0, Vec::len)
    }

    pub fn temperature(self, index: usize) -> Option<TemperaturePanel<'a>> {
        let value = self.state.snapshot.temperatures.value()?.get(index)?;
        Some(TemperaturePanel {
            value,
            history: self.state.history.temperatures.get(&value.id),
        })
    }

    pub fn gpu_memory_count(self) -> usize {
        self.state.snapshot.gpu_memory.value().map_or(0, Vec::len)
    }

    pub fn gpu_memory(self, index: usize) -> Option<GpuMemoryPanel<'a>> {
        let value = self.state.snapshot.gpu_memory.value()?.get(index)?;
        Some(GpuMemoryPanel {
            value,
            history: self.state.history.gpu_memory_used.get(&value.id),
        })
    }

    pub fn disk_count(self) -> usize {
        self.state.snapshot.disks.value().map_or(0, Vec::len)
    }

    pub fn disk(self, index: usize) -> Option<DiskPanel<'a>> {
        let value = self.state.snapshot.disks.value()?.get(index)?;
        Some(DiskPanel {
            value,
            history: self.state.history.disks.get(&value.id),
            processes: &self.state.snapshot.process_disk_io,
        })
    }

    pub fn network_count(self) -> usize {
        self.state.snapshot.networks.value().map_or(0, Vec::len)
    }

    pub fn network(self, index: usize) -> Option<NetworkPanel<'a>> {
        let value = self.state.snapshot.networks.value()?.get(index)?;
        Some(NetworkPanel {
            value,
            history: self.state.history.networks.get(&value.id),
            processes: &self.state.snapshot.process_network_io,
        })
    }
}

#[derive(Clone, Copy)]
pub struct MemoryPanel<'a> {
    memory: Option<&'a MemorySnapshot>,
    processes: &'a [ProcessMemoryUsage],
    history: &'a VecDeque<f64>,
}

impl<'a> MemoryPanel<'a> {
    pub const TITLE: &'static str = "MEMORY";
    pub const SECONDARY_LABEL: &'static str = "SWAP";

    pub fn value(self) -> String {
        self.memory
            .map(|memory| format_bytes(memory.used_bytes))
            .unwrap_or_else(unavailable)
    }

    pub fn subtitle(self) -> String {
        self.memory
            .map(|memory| format!("/ {}", format_bytes(memory.total_bytes)))
            .unwrap_or_default()
    }

    pub fn secondary_value(self) -> String {
        self.memory
            .map(|memory| {
                format!(
                    "{} / {}",
                    format_bytes(memory.swap_used_bytes),
                    format_bytes(memory.swap_total_bytes)
                )
            })
            .unwrap_or_else(unavailable)
    }

    pub fn fraction(self) -> f32 {
        match self.memory {
            Some(memory) if memory.total_bytes > 0 => {
                memory.used_bytes as f32 / memory.total_bytes as f32
            }
            _ => 0.0,
        }
    }

    pub fn graph_values(self) -> &'a VecDeque<f64> {
        self.history
    }

    pub fn graph_max(self) -> f64 {
        self.memory
            .map(|memory| memory.total_bytes.max(1) as f64)
            .unwrap_or(1.0)
    }

    pub fn process_count(self) -> usize {
        self.processes.len()
    }

    pub fn processes(self) -> impl Iterator<Item = ProcessRow<'a>> + 'a {
        self.processes.iter().map(|process| ProcessRow {
            name: &process.name,
            value: format_bytes(process.bytes),
        })
    }
}

#[derive(Clone, Copy)]
pub struct CpuPanel<'a> {
    percent: Option<f64>,
    processes: &'a [ProcessCpuUsage],
    history: &'a VecDeque<f64>,
}

impl<'a> CpuPanel<'a> {
    pub const TITLE: &'static str = "CPU";

    pub fn value(self) -> String {
        self.percent.map(format_percent).unwrap_or_else(unavailable)
    }

    pub fn fraction(self) -> f32 {
        self.percent.unwrap_or(0.0) as f32 / 100.0
    }

    pub fn graph_values(self) -> &'a VecDeque<f64> {
        self.history
    }

    pub fn process_count(self) -> usize {
        self.processes.len()
    }

    pub fn processes(self) -> impl Iterator<Item = ProcessRow<'a>> + 'a {
        self.processes.iter().map(|process| ProcessRow {
            name: &process.name,
            value: format_percent(process.percent),
        })
    }
}

pub struct ProcessRow<'a> {
    pub name: &'a str,
    pub value: String,
}

#[derive(Clone, Copy)]
pub struct GpuMemoryPanel<'a> {
    value: &'a GpuMemorySnapshot,
    history: Option<&'a VecDeque<f64>>,
}

impl<'a> GpuMemoryPanel<'a> {
    pub fn name(self) -> &'a str {
        short_gpu_name(&self.value.name)
    }

    pub fn value(self) -> String {
        format!(
            "{} / {}",
            format_bytes(self.value.used_bytes),
            format_bytes(self.value.total_bytes)
        )
    }

    pub fn percent(self) -> String {
        format_percent(f64::from(self.fraction()) * 100.0)
    }

    pub fn fraction(self) -> f32 {
        if self.value.total_bytes == 0 {
            0.0
        } else {
            (self.value.used_bytes as f32 / self.value.total_bytes as f32).clamp(0.0, 1.0)
        }
    }

    pub fn history(self) -> Option<&'a VecDeque<f64>> {
        self.history
    }

    pub fn graph_max(self) -> f64 {
        self.value.total_bytes.max(1) as f64
    }
}

#[derive(Clone, Copy)]
pub struct TemperaturePanel<'a> {
    value: &'a TemperatureSnapshot,
    history: Option<&'a VecDeque<f64>>,
}

impl<'a> TemperaturePanel<'a> {
    pub fn name(self) -> &'a str {
        short_temperature_name(&self.value.name)
    }

    pub fn value(self) -> String {
        format!("{:.1}°C", self.value.celsius)
    }

    pub fn history(self) -> Option<&'a VecDeque<f64>> {
        self.history
    }
}

fn short_temperature_name(name: &str) -> &str {
    short_gpu_name(name)
}

fn short_gpu_name(name: &str) -> &str {
    let name = name.strip_prefix("NVIDIA ").unwrap_or(name);
    name.strip_prefix("GeForce ").unwrap_or(name)
}

#[derive(Clone, Copy)]
pub struct DiskPanel<'a> {
    value: &'a DiskSnapshot,
    history: Option<&'a VecDeque<f64>>,
    processes: &'a Collection<Vec<ProcessDiskIo>>,
}

impl<'a> DiskPanel<'a> {
    pub fn name(self) -> &'a str {
        &self.value.name
    }

    pub fn value(self) -> String {
        format_rate(self.value.bytes_per_sec)
    }

    pub fn history(self) -> Option<&'a VecDeque<f64>> {
        self.history
    }

    pub fn process_unavailable_value(self) -> Option<&'static str> {
        (!self.processes.is_observable()).then_some(UNAVAILABLE_VALUE)
    }

    pub fn processes(self) -> impl Iterator<Item = IoProcessRow<'a>> + 'a {
        let disk_id = &self.value.id;
        self.processes
            .value()
            .into_iter()
            .flatten()
            .filter(move |process| &process.disk_id == disk_id)
            .filter(|process| process.read_bytes_per_sec + process.write_bytes_per_sec > 0.0)
            .map(|process| IoProcessRow {
                name: process.name.as_deref(),
                pid: process.process.pid,
                first_value: format_compact_rate(process.read_bytes_per_sec),
                second_value: format_compact_rate(process.write_bytes_per_sec),
            })
    }
}

#[derive(Clone, Copy)]
pub struct NetworkPanel<'a> {
    value: &'a NetworkSnapshot,
    history: Option<&'a NetworkDirectionHistory>,
    processes: &'a Collection<Vec<ProcessNetworkIo>>,
}

impl<'a> NetworkPanel<'a> {
    pub fn name(self) -> &'a str {
        &self.value.name
    }

    pub fn down_value(self) -> String {
        format_rate(self.value.down_bytes_per_sec)
    }

    pub fn up_value(self) -> String {
        format_rate(self.value.up_bytes_per_sec)
    }

    pub fn history(self) -> Option<&'a NetworkDirectionHistory> {
        self.history
    }

    pub fn process_unavailable_value(self) -> Option<&'static str> {
        (!self.processes.is_observable()).then_some(UNAVAILABLE_VALUE)
    }

    pub fn processes(self) -> impl Iterator<Item = IoProcessRow<'a>> + 'a {
        let network_id = &self.value.id;
        self.processes
            .value()
            .into_iter()
            .flatten()
            .filter(move |process| &process.network_id == network_id)
            .filter(|process| process.rx_bytes_per_sec + process.tx_bytes_per_sec > 0.0)
            .map(|process| IoProcessRow {
                name: process.name.as_deref(),
                pid: process.process.pid,
                first_value: format_compact_rate(process.rx_bytes_per_sec),
                second_value: format_compact_rate(process.tx_bytes_per_sec),
            })
    }
}

pub struct IoProcessRow<'a> {
    pub name: Option<&'a str>,
    pub pid: u32,
    pub first_value: String,
    pub second_value: String,
}

const UNAVAILABLE_VALUE: &str = "—";

pub fn unavailable() -> String {
    UNAVAILABLE_VALUE.to_owned()
}

pub fn format_rate(value: f64) -> String {
    format!("{}/s", format_bytes(value.max(0.0) as u64))
}

fn format_compact_rate(value: f64) -> String {
    const UNITS: [&str; 5] = ["B/s", "K/s", "M/s", "G/s", "T/s"];
    let mut number = value.max(0.0);
    let mut unit = 0;
    while number >= 1024.0 && unit < UNITS.len() - 1 {
        number /= 1024.0;
        unit += 1;
    }
    let formatted = if unit == 0 || number >= 10.0 {
        format!("{number:.0}")
    } else {
        format!("{number:.1}")
    };
    format!("{formatted}{}", UNITS[unit])
}

pub fn format_percent(value: f64) -> String {
    format!("{value:.1}%")
}

pub fn format_bytes(value: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut number = value as f64;
    let mut unit = 0;
    while number >= 1024.0 && unit < UNITS.len() - 1 {
        number /= 1024.0;
        unit += 1;
    }
    let formatted = if unit == 0 || number >= 100.0 {
        format!("{number:.0}")
    } else if number >= 10.0 {
        format!("{number:.1}")
    } else {
        format!("{number:.2}")
    };
    format!("{formatted} {}", UNITS[unit])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_value_formatting_is_stable() {
        assert_eq!(format_bytes(999), "999 B");
        assert_eq!(format_bytes(1024), "1.00 KiB");
        assert_eq!(format_bytes(10 * 1024), "10.0 KiB");
        assert_eq!(format_rate(1024.0), "1.00 KiB/s");
        assert_eq!(format_compact_rate(1024.0), "1.0K/s");
        assert_eq!(format_percent(12.34), "12.3%");
        assert_eq!(unavailable(), "—");
    }

    #[test]
    fn gpu_memory_panel_formats_capacity_and_fraction() {
        let mut state = MonitorState::default();
        state.snapshot.gpu_memory = Collection::available(vec![GpuMemorySnapshot {
            id: cclover_core::model::GpuId::from_opaque_key("gpu-a"),
            name: "NVIDIA GeForce RTX Test".into(),
            used_bytes: 4 * 1024 * 1024 * 1024,
            total_bytes: 8 * 1024 * 1024 * 1024,
        }]);

        let gpu_id = state.snapshot.gpu_memory.value().unwrap()[0].id.clone();
        state
            .history
            .gpu_memory_used
            .insert(gpu_id, VecDeque::from([1.0, 2.0, 3.0]));

        let panel = Dashboard::new(&state).gpu_memory(0).unwrap();
        assert_eq!(panel.name(), "RTX Test");
        assert_eq!(panel.value(), "4.00 GiB / 8.00 GiB");
        assert_eq!(panel.percent(), "50.0%");
        assert_eq!(panel.fraction(), 0.5);
        assert_eq!(panel.history().unwrap(), &VecDeque::from([1.0, 2.0, 3.0]));
        assert_eq!(panel.graph_max(), (8_u64 * 1024 * 1024 * 1024) as f64);
    }

    #[test]
    fn unavailable_attribution_does_not_hide_available_parent_metric() {
        let disk_id = cclover_core::model::DiskId::from_opaque_key("disk-a");
        let mut state = MonitorState::default();
        state.snapshot.disks = Collection::available(vec![DiskSnapshot {
            id: disk_id,
            name: "nvme0n1".into(),
            bytes_per_sec: 10.0,
        }]);
        state.snapshot.process_disk_io =
            Collection::unavailable(cclover_core::model::CollectionUnavailable::PermissionDenied);

        let panel = Dashboard::new(&state).disk(0).unwrap();
        assert_eq!(panel.name(), "nvme0n1");
        assert_eq!(panel.process_unavailable_value(), Some(UNAVAILABLE_VALUE));
        assert_eq!(panel.processes().count(), 0);

        state.snapshot.process_disk_io = Collection::available(Vec::new());
        let panel = Dashboard::new(&state).disk(0).unwrap();
        assert_eq!(panel.process_unavailable_value(), None);
        assert_eq!(panel.processes().count(), 0);
    }

    #[test]
    fn io_process_rows_match_stable_device_identity_and_hide_zero_rate() {
        let disk_id = cclover_core::model::DiskId::from_opaque_key("disk-a");
        let other_disk_id = cclover_core::model::DiskId::from_opaque_key("disk-b");
        let network_id = cclover_core::model::NetworkId::from_opaque_key("network-a");
        let other_network_id = cclover_core::model::NetworkId::from_opaque_key("network-b");
        let process = cclover_core::model::ProcessInstanceId {
            pid: 42,
            birth_marker: 7,
        };
        let mut state = MonitorState::default();
        state.snapshot.disks = Collection::available(vec![DiskSnapshot {
            id: disk_id.clone(),
            name: "nvme0n1".into(),
            bytes_per_sec: 0.0,
        }]);
        state.snapshot.networks = Collection::available(vec![NetworkSnapshot {
            id: network_id.clone(),
            name: "eth0".into(),
            down_bytes_per_sec: 0.0,
            up_bytes_per_sec: 0.0,
        }]);
        state.snapshot.process_disk_io = Collection::available(vec![
            ProcessDiskIo {
                process,
                name: Some("worker".into()),
                disk_id: disk_id.clone(),
                device: "renamed-display-label".into(),
                read_bytes_per_sec: 2048.0,
                write_bytes_per_sec: 1024.0,
            },
            ProcessDiskIo {
                process,
                name: Some("idle".into()),
                disk_id,
                device: "nvme0n1".into(),
                read_bytes_per_sec: 0.0,
                write_bytes_per_sec: 0.0,
            },
            ProcessDiskIo {
                process,
                name: Some("other".into()),
                disk_id: other_disk_id,
                device: "nvme1n1".into(),
                read_bytes_per_sec: 4096.0,
                write_bytes_per_sec: 4096.0,
            },
        ]);
        state.snapshot.process_network_io = Collection::available(vec![
            ProcessNetworkIo {
                process,
                name: Some("worker".into()),
                network_id: network_id.clone(),
                interface: "renamed-display-label".into(),
                rx_bytes_per_sec: 3072.0,
                tx_bytes_per_sec: 1024.0,
            },
            ProcessNetworkIo {
                process,
                name: Some("idle".into()),
                network_id,
                interface: "eth0".into(),
                rx_bytes_per_sec: 0.0,
                tx_bytes_per_sec: 0.0,
            },
            ProcessNetworkIo {
                process,
                name: Some("other".into()),
                network_id: other_network_id,
                interface: "eth1".into(),
                rx_bytes_per_sec: 4096.0,
                tx_bytes_per_sec: 4096.0,
            },
        ]);

        let disk_rows: Vec<_> = Dashboard::new(&state)
            .disk(0)
            .unwrap()
            .processes()
            .collect();
        let network_rows: Vec<_> = Dashboard::new(&state)
            .network(0)
            .unwrap()
            .processes()
            .collect();

        assert_eq!(disk_rows.len(), 1);
        assert_eq!(disk_rows[0].name, Some("worker"));
        assert_eq!(disk_rows[0].pid, 42);
        assert_eq!(disk_rows[0].first_value, "2.0K/s");
        assert_eq!(disk_rows[0].second_value, "1.0K/s");
        assert_eq!(network_rows.len(), 1);
        assert_eq!(network_rows[0].name, Some("worker"));
        assert_eq!(network_rows[0].pid, 42);
        assert_eq!(network_rows[0].first_value, "3.0K/s");
        assert_eq!(network_rows[0].second_value, "1.0K/s");
    }

    #[test]
    fn nvidia_temperature_names_drop_only_known_redundant_prefixes() {
        assert_eq!(
            short_temperature_name("NVIDIA GeForce RTX 2080 Ti"),
            "RTX 2080 Ti"
        );
        assert_eq!(
            short_temperature_name("NVIDIA GeForce GTX 1080"),
            "GTX 1080"
        );
        assert_eq!(short_temperature_name("NVIDIA RTX A4000"), "RTX A4000");
        assert_eq!(
            short_temperature_name("NVIDIA A100-PCIE-40GB"),
            "A100-PCIE-40GB"
        );
        assert_eq!(short_temperature_name("amdgpu"), "amdgpu");
        assert_eq!(short_temperature_name("CPU"), "CPU");
    }
}
