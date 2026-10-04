use cclover_presentation::{
    BoundedText, CpuPanel, DISK_SECTION, Dashboard, FAN_SECTION, GPU_SECTION, MemoryPanel,
    NETWORK_SECTION, TEMPERATURE_SECTION,
};

use crate::cards::{
    EMPTY_GRAPH_VALUES, MetricCardParams, disk_card, gpu_card, metric_card, network_card, section,
    small_graph_card,
};
use crate::{
    Block, GraphSpec, METRIC_CARD_GEOMETRY, PANEL_GEOMETRY, SMALL_GRAPH_CARD_GEOMETRY, Tone,
};

#[derive(Debug, Clone)]
pub struct DashboardUi<'a> {
    pub left: Vec<Block<'a>>,
    pub right: Vec<Block<'a>>,
}

impl<'a> DashboardUi<'a> {
    pub fn new(dashboard: Dashboard<'a>) -> Self {
        let capacity = dashboard.history_capacity();
        let memory = dashboard.memory();
        let cpu = dashboard.cpu();

        let mut left = vec![metric_card(MetricCardParams {
            title: MemoryPanel::TITLE,
            value: memory.compact_value(),
            subtitle: memory.compact_subtitle(),
            secondary_label: MemoryPanel::SECONDARY_LABEL,
            secondary_value: memory.compact_secondary_value(),
            progress: memory.fraction(),
            graph: GraphSpec {
                values: memory.graph_values(),
                min: 0.0,
                max: memory.graph_max(),
                auto_scale: false,
                line: Tone::Green,
                fill_alpha: 0.13,
                capacity,
                height: METRIC_CARD_GEOMETRY.graph_height,
            },
            processes: memory
                .processes()
                .map(|row| (row.name, row.compact_value))
                .collect(),
        })];
        left.push(section(GPU_SECTION));
        for index in 0..dashboard.gpu_count() {
            let gpu = dashboard
                .gpu(index)
                .expect("dashboard GPU count and lookup must agree");
            left.push(gpu_card(gpu, capacity));
        }
        left.push(section(TEMPERATURE_SECTION));
        for index in 0..dashboard.temperature_count() {
            let temperature = dashboard
                .temperature(index)
                .expect("dashboard temperature count and lookup must agree");
            left.push(small_graph_card(
                temperature.name(),
                temperature.compact_value(),
                GraphSpec {
                    values: temperature.history().unwrap_or(&EMPTY_GRAPH_VALUES),
                    min: 20.0,
                    max: 100.0,
                    auto_scale: false,
                    line: Tone::Red,
                    fill_alpha: 0.13,
                    capacity,
                    height: SMALL_GRAPH_CARD_GEOMETRY.graph_height,
                },
            ));
        }
        left.push(section(FAN_SECTION));
        for index in 0..dashboard.fan_count() {
            let fan = dashboard
                .fan(index)
                .expect("dashboard fan count and lookup must agree");
            left.push(small_graph_card(
                fan.name(),
                fan.compact_value(),
                GraphSpec {
                    values: fan.history().unwrap_or(&EMPTY_GRAPH_VALUES),
                    min: 0.0,
                    max: 1.0,
                    auto_scale: true,
                    line: Tone::Accent,
                    fill_alpha: 0.10,
                    capacity,
                    height: SMALL_GRAPH_CARD_GEOMETRY.graph_height,
                },
            ));
        }

        let mut right = vec![metric_card(MetricCardParams {
            title: CpuPanel::TITLE,
            value: cpu.compact_value(),
            subtitle: BoundedText::default(),
            secondary_label: "",
            secondary_value: BoundedText::default(),
            progress: cpu.fraction(),
            graph: GraphSpec {
                values: cpu.graph_values(),
                min: 0.0,
                max: 100.0,
                auto_scale: false,
                line: Tone::Accent,
                fill_alpha: 0.14,
                capacity,
                height: METRIC_CARD_GEOMETRY.graph_height,
            },
            processes: cpu
                .processes()
                .map(|row| (row.name, row.compact_value))
                .collect(),
        })];
        right.push(section(DISK_SECTION));
        for index in 0..dashboard.disk_count() {
            let disk = dashboard
                .disk(index)
                .expect("dashboard disk count and lookup must agree");
            right.push(disk_card(disk, capacity));
        }
        right.push(section(NETWORK_SECTION));
        for index in 0..dashboard.network_count() {
            let network = dashboard
                .network(index)
                .expect("dashboard network count and lookup must agree");
            right.push(network_card(network, capacity));
        }

        Self { left, right }
    }

    pub fn height(&self) -> u32 {
        column_height(&self.left)
            .max(column_height(&self.right))
            .saturating_add(PANEL_GEOMETRY.padding * 2)
            .max(PANEL_GEOMETRY.min_height)
    }
}

fn column_height(blocks: &[Block<'_>]) -> u32 {
    blocks.iter().map(Block::height).sum::<u32>()
        + PANEL_GEOMETRY.column_spacing * blocks.len().saturating_sub(1) as u32
}
