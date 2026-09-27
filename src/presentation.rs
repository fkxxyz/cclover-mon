use std::collections::VecDeque;

use crate::core::model::{
    DirectionHistory, DiskSnapshot, MemorySnapshot, MonitorState, NetworkSnapshot, ProcessCpu,
    ProcessMemory, TemperatureSnapshot,
};

pub const TEMPERATURE_SECTION: &str = "TEMPERATURE";
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
            memory: self.state.snapshot.memory.as_ref(),
            processes: &self.state.snapshot.top_memory,
            history: &self.state.history.memory_used,
        }
    }

    pub fn cpu(self) -> CpuPanel<'a> {
        CpuPanel {
            percent: self.state.snapshot.cpu_percent,
            processes: &self.state.snapshot.top_cpu,
            history: &self.state.history.cpu,
        }
    }

    pub fn temperature_count(self) -> usize {
        self.state.snapshot.temperatures.len()
    }

    pub fn temperature(self, index: usize) -> Option<TemperaturePanel<'a>> {
        let value = self.state.snapshot.temperatures.get(index)?;
        Some(TemperaturePanel {
            value,
            history: self.state.history.temperatures.get(&value.name),
        })
    }

    pub fn disk_count(self) -> usize {
        self.state.snapshot.disks.len()
    }

    pub fn disk(self, index: usize) -> Option<DiskPanel<'a>> {
        let value = self.state.snapshot.disks.get(index)?;
        Some(DiskPanel {
            value,
            history: self.state.history.disks.get(&value.name),
        })
    }

    pub fn network_count(self) -> usize {
        self.state.snapshot.networks.len()
    }

    pub fn network(self, index: usize) -> Option<NetworkPanel<'a>> {
        let value = self.state.snapshot.networks.get(index)?;
        Some(NetworkPanel {
            value,
            history: self.state.history.networks.get(&value.name),
        })
    }
}

#[derive(Clone, Copy)]
pub struct MemoryPanel<'a> {
    memory: Option<&'a MemorySnapshot>,
    processes: &'a [ProcessMemory],
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
    processes: &'a [ProcessCpu],
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
pub struct TemperaturePanel<'a> {
    value: &'a TemperatureSnapshot,
    history: Option<&'a VecDeque<f64>>,
}

impl<'a> TemperaturePanel<'a> {
    pub fn name(self) -> &'a str {
        &self.value.name
    }

    pub fn value(self) -> String {
        format!("{:.1}°C", self.value.celsius)
    }

    pub fn history(self) -> Option<&'a VecDeque<f64>> {
        self.history
    }
}

#[derive(Clone, Copy)]
pub struct DiskPanel<'a> {
    value: &'a DiskSnapshot,
    history: Option<&'a VecDeque<f64>>,
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
}

#[derive(Clone, Copy)]
pub struct NetworkPanel<'a> {
    value: &'a NetworkSnapshot,
    history: Option<&'a DirectionHistory>,
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

    pub fn history(self) -> Option<&'a DirectionHistory> {
        self.history
    }
}

pub fn unavailable() -> String {
    "—".to_owned()
}

pub fn format_rate(value: f64) -> String {
    format!("{}/s", format_bytes(value.max(0.0) as u64))
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
        assert_eq!(format_percent(12.34), "12.3%");
        assert_eq!(unavailable(), "—");
    }
}
