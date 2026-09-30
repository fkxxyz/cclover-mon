#![deny(unsafe_code)]

use std::io;
use std::time::Duration;

use cclover_presentation::{CpuPanel, Dashboard, MemoryPanel};

mod native;

use native::{Frame, Panel, Terminal};

pub struct TerminalUi {
    terminal: Terminal,
}

impl TerminalUi {
    pub fn enter() -> io::Result<Self> {
        Ok(Self {
            terminal: Terminal::enter()?,
        })
    }

    pub fn draw(&mut self, dashboard: Dashboard<'_>) -> io::Result<()> {
        let frame = build_frame(dashboard);
        self.terminal.draw(&frame)
    }

    pub fn size_changed(&mut self) -> io::Result<bool> {
        self.terminal.size_changed()
    }

    pub fn wait_for_quit(&mut self, wait: Duration) -> io::Result<bool> {
        self.terminal.wait_for_quit(wait)
    }
}

fn build_frame(dashboard: Dashboard<'_>) -> Frame {
    Frame {
        cpu: cpu_panel(dashboard.cpu()),
        memory: memory_panel(dashboard.memory()),
        gpu: Panel {
            title: "GPU".to_owned(),
            rows: (0..dashboard.gpu_count())
                .filter_map(|index| dashboard.gpu(index))
                .map(|gpu| {
                    format!(
                        "{}  {}  {}  {}  {}  {}",
                        gpu.name(),
                        gpu.utilization_value(),
                        gpu.memory_value(),
                        gpu.temperature_value(),
                        gpu.power_value(),
                        gpu.core_clock_value(),
                    )
                })
                .collect(),
            ..Panel::default()
        },
        temperatures: Panel {
            title: "TEMPERATURE".to_owned(),
            rows: (0..dashboard.temperature_count())
                .filter_map(|index| dashboard.temperature(index))
                .map(|temperature| format!("{}  {}", temperature.name(), temperature.value()))
                .collect(),
            ..Panel::default()
        },
        fans: Panel {
            title: "FAN".to_owned(),
            rows: (0..dashboard.fan_count())
                .filter_map(|index| dashboard.fan(index))
                .map(|fan| format!("{}  {}", fan.name(), fan.value()))
                .collect(),
            ..Panel::default()
        },
        disks: Panel {
            title: "DISK I/O".to_owned(),
            rows: disk_rows(dashboard),
            ..Panel::default()
        },
        networks: Panel {
            title: "NETWORK".to_owned(),
            rows: network_rows(dashboard),
            ..Panel::default()
        },
    }
}

fn cpu_panel(panel: CpuPanel<'_>) -> Panel {
    Panel {
        title: format!("CPU  {}", panel.value()),
        rows: panel
            .processes()
            .map(|process| format!("{}  {}", process.name, process.value))
            .collect(),
        history: sparkline_values(panel.graph_values()),
    }
}

fn memory_panel(panel: MemoryPanel<'_>) -> Panel {
    let mut rows = vec![format!(
        "{}  {}",
        MemoryPanel::SECONDARY_LABEL,
        panel.secondary_value()
    )];
    rows.extend(
        panel
            .processes()
            .map(|process| format!("{}  {}", process.name, process.value)),
    );
    Panel {
        title: format!("MEMORY  {} {}", panel.value(), panel.subtitle()),
        rows,
        history: sparkline_values(panel.graph_values()),
    }
}

fn disk_rows(dashboard: Dashboard<'_>) -> Vec<String> {
    let mut lines = Vec::new();
    for index in 0..dashboard.disk_count() {
        let Some(disk) = dashboard.disk(index) else {
            continue;
        };
        lines.push(format!("{}  {}", disk.name(), disk.value()));
        lines.extend(disk.processes().map(|process| {
            format!(
                "  {}  R {}  W {}",
                process.name.unwrap_or("?"),
                process.first_value,
                process.second_value
            )
        }));
    }
    lines
}

fn network_rows(dashboard: Dashboard<'_>) -> Vec<String> {
    let mut lines = Vec::new();
    for index in 0..dashboard.network_count() {
        let Some(network) = dashboard.network(index) else {
            continue;
        };
        lines.push(format!(
            "{}  ↓ {}  ↑ {}",
            network.name(),
            network.down_value(),
            network.up_value()
        ));
        lines.extend(network.processes().map(|process| {
            format!(
                "  {}  ↓ {}  ↑ {}",
                process.name.unwrap_or("?"),
                process.first_value,
                process.second_value
            )
        }));
    }
    lines
}

fn sparkline_values(values: &std::collections::VecDeque<f64>) -> Vec<u64> {
    values
        .iter()
        .map(|value| value.max(0.0).round() as u64)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparkline_clamps_negative_values() {
        let values = [-1.0, 0.0, 1.4, 2.6].into_iter().collect();
        assert_eq!(sparkline_values(&values), vec![0, 0, 1, 3]);
    }
}
