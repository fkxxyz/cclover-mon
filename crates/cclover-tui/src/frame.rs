use cclover_presentation::{CpuPanel, Dashboard, MemoryPanel};

#[derive(Default)]
pub(crate) struct Frame {
    pub(crate) cpu: Overview,
    pub(crate) memory: Overview,
    pub(crate) gpus: Vec<GpuRow>,
    pub(crate) thermals: Vec<Metric>,
    pub(crate) disks: Vec<IoDevice>,
    pub(crate) networks: Vec<NetworkDevice>,
}

#[derive(Default)]
pub(crate) struct Overview {
    pub(crate) title: &'static str,
    pub(crate) value: String,
    pub(crate) secondary: Option<Metric>,
    pub(crate) processes: Vec<Metric>,
    pub(crate) history: Vec<f64>,
    pub(crate) history_max: f64,
}

pub(crate) struct Metric {
    pub(crate) name: String,
    pub(crate) value: String,
}

pub(crate) struct GpuRow {
    pub(crate) name: String,
    pub(crate) utilization: String,
    pub(crate) memory: String,
    pub(crate) temperature: String,
    pub(crate) power: String,
    pub(crate) clock: String,
}

pub(crate) struct IoProcess {
    pub(crate) name: String,
    pub(crate) first: String,
    pub(crate) second: String,
}

pub(crate) struct IoDevice {
    pub(crate) name: String,
    pub(crate) rate: String,
    pub(crate) processes: Vec<IoProcess>,
}

pub(crate) struct NetworkDevice {
    pub(crate) name: String,
    pub(crate) down: String,
    pub(crate) up: String,
    pub(crate) processes: Vec<IoProcess>,
}

pub(crate) fn build(dashboard: Dashboard<'_>) -> Frame {
    Frame {
        cpu: cpu(dashboard.cpu()),
        memory: memory(dashboard.memory()),
        gpus: (0..dashboard.gpu_count())
            .filter_map(|index| dashboard.gpu(index))
            .map(|gpu| GpuRow {
                name: gpu.name().to_owned(),
                utilization: gpu.utilization_value(),
                memory: gpu.memory_value(),
                temperature: gpu.temperature_value(),
                power: gpu.power_value(),
                clock: gpu.core_clock_value(),
            })
            .collect(),
        thermals: thermal_rows(dashboard),
        disks: disk_rows(dashboard),
        networks: network_rows(dashboard),
    }
}

fn cpu(panel: CpuPanel<'_>) -> Overview {
    Overview {
        title: CpuPanel::TITLE,
        value: panel.value(),
        processes: panel
            .processes()
            .map(|process| Metric {
                name: process.name.to_owned(),
                value: process.value,
            })
            .collect(),
        history: panel.graph_values().iter().copied().collect(),
        history_max: 100.0,
        ..Overview::default()
    }
}

fn memory(panel: MemoryPanel<'_>) -> Overview {
    Overview {
        title: MemoryPanel::TITLE,
        value: format!("{} {}", panel.value(), panel.subtitle()),
        secondary: Some(Metric {
            name: MemoryPanel::SECONDARY_LABEL.to_owned(),
            value: panel.secondary_value(),
        }),
        processes: panel
            .processes()
            .map(|process| Metric {
                name: process.name.to_owned(),
                value: process.value,
            })
            .collect(),
        history: panel.graph_values().iter().copied().collect(),
        history_max: panel.graph_max(),
    }
}

fn thermal_rows(dashboard: Dashboard<'_>) -> Vec<Metric> {
    let mut rows = Vec::new();
    rows.extend(
        (0..dashboard.temperature_count())
            .filter_map(|index| dashboard.temperature(index))
            .map(|temperature| Metric {
                name: temperature.name().to_owned(),
                value: temperature.value(),
            }),
    );
    rows.extend(
        (0..dashboard.fan_count())
            .filter_map(|index| dashboard.fan(index))
            .map(|fan| Metric {
                name: fan.name().to_owned(),
                value: fan.value(),
            }),
    );
    rows
}

fn disk_rows(dashboard: Dashboard<'_>) -> Vec<IoDevice> {
    (0..dashboard.disk_count())
        .filter_map(|index| dashboard.disk(index))
        .map(|disk| IoDevice {
            name: disk.name(),
            rate: disk.value(),
            processes: disk
                .processes()
                .map(|process| IoProcess {
                    name: process
                        .name
                        .map(str::to_owned)
                        .unwrap_or_else(|| format!("pid {}", process.pid)),
                    first: process.first_value,
                    second: process.second_value,
                })
                .collect(),
        })
        .collect()
}

fn network_rows(dashboard: Dashboard<'_>) -> Vec<NetworkDevice> {
    (0..dashboard.network_count())
        .filter_map(|index| dashboard.network(index))
        .map(|network| NetworkDevice {
            name: network.name().to_owned(),
            down: network.down_value(),
            up: network.up_value(),
            processes: network
                .processes()
                .map(|process| IoProcess {
                    name: process
                        .name
                        .map(str::to_owned)
                        .unwrap_or_else(|| format!("pid {}", process.pid)),
                    first: process.first_value,
                    second: process.second_value,
                })
                .collect(),
        })
        .collect()
}
