use std::borrow::Cow;
use std::collections::VecDeque;

use cclover_presentation::{
    CpuPanel, DISK_SECTION, Dashboard, FAN_SECTION, GPU_SECTION, MemoryPanel, NETWORK_SECTION,
    TEMPERATURE_SECTION,
};

mod scene;
pub use scene::{CacheClass, NativeScene, NativeTextMeasurer, Point, Primitive, Rect, TextAlign};

pub const PANEL_WIDTH: u32 = 390;
pub const INITIAL_PANEL_HEIGHT: u32 = 480;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Background,
    Card,
    Border,
    Foreground,
    Muted,
    Accent,
    Green,
    Orange,
    Red,
    Guide,
}

impl Tone {
    pub const fn rgba(self) -> Rgba {
        match self {
            Self::Background => Rgba {
                r: 0x0b,
                g: 0x10,
                b: 0x20,
                a: 0.85,
            },
            Self::Card => Rgba {
                r: 0x15,
                g: 0x1b,
                b: 0x2d,
                a: 0.80,
            },
            Self::Border => Rgba {
                r: 0x33,
                g: 0x41,
                b: 0x5f,
                a: 1.0,
            },
            Self::Foreground => Rgba {
                r: 0xf3,
                g: 0xf6,
                b: 0xff,
                a: 1.0,
            },
            Self::Muted => Rgba {
                r: 0x8f,
                g: 0x9b,
                b: 0xb0,
                a: 1.0,
            },
            Self::Accent => Rgba {
                r: 0x7c,
                g: 0x9c,
                b: 0xff,
                a: 1.0,
            },
            Self::Green => Rgba {
                r: 0x52,
                g: 0xe0,
                b: 0xc4,
                a: 1.0,
            },
            Self::Orange => Rgba {
                r: 0xff,
                g: 0xb8,
                b: 0x6b,
                a: 1.0,
            },
            Self::Red => Rgba {
                r: 0xff,
                g: 0x7e,
                b: 0x9b,
                a: 1.0,
            },
            Self::Guide => Rgba {
                r: 0x26,
                g: 0x34,
                b: 0x4e,
                a: 1.0,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextWeight {
    Regular,
    Bold,
}

#[derive(Debug, Clone)]
pub struct TextCell<'a> {
    pub text: Cow<'a, str>,
    pub size: u32,
    pub tone: Tone,
    pub weight: TextWeight,
    pub grow: bool,
    pub clip: bool,
    pub static_content: bool,
}

impl<'a> TextCell<'a> {
    fn borrowed(text: &'a str, size: u32, tone: Tone) -> Self {
        Self {
            text: Cow::Borrowed(text),
            size,
            tone,
            weight: TextWeight::Regular,
            grow: false,
            clip: false,
            static_content: false,
        }
    }

    fn owned(text: String, size: u32, tone: Tone) -> Self {
        Self {
            text: Cow::Owned(text),
            size,
            tone,
            weight: TextWeight::Regular,
            grow: false,
            clip: false,
            static_content: false,
        }
    }

    fn bold(mut self) -> Self {
        self.weight = TextWeight::Bold;
        self
    }

    fn grow(mut self) -> Self {
        self.grow = true;
        self
    }

    fn clip(mut self) -> Self {
        self.clip = true;
        self
    }

    fn static_content(mut self) -> Self {
        self.static_content = true;
        self
    }
}

#[derive(Debug, Clone)]
pub struct TextRow<'a> {
    pub cells: Vec<TextCell<'a>>,
    pub height: u32,
    pub gap: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct GraphSpec<'a> {
    pub values: &'a VecDeque<f64>,
    pub min: f64,
    pub max: f64,
    pub auto_scale: bool,
    pub line: Tone,
    pub fill_alpha: f32,
    pub capacity: usize,
    pub height: u32,
}

impl GraphSpec<'_> {
    pub fn resolved_max(&self) -> f64 {
        if self.auto_scale {
            (self.values.iter().copied().fold(1024.0_f64, f64::max) * 1.12).max(self.min + 0.001)
        } else {
            self.max.max(self.min + 0.001)
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ProgressSpec {
    pub value: f32,
    pub tone: Tone,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct Stack<'a> {
    pub children: Vec<Element<'a>>,
    pub gap: u32,
    pub height: Option<u32>,
}

#[derive(Debug, Clone)]
pub enum Element<'a> {
    Row(TextRow<'a>),
    Graph(GraphSpec<'a>),
    Progress(ProgressSpec),
    Stack(Stack<'a>),
}

impl Element<'_> {
    pub fn height(&self) -> u32 {
        match self {
            Self::Row(row) => row.height,
            Self::Graph(graph) => graph.height,
            Self::Progress(progress) => progress.height,
            Self::Stack(stack) => stack.height.unwrap_or_else(|| stack.content_height()),
        }
    }
}

impl Stack<'_> {
    pub fn content_height(&self) -> u32 {
        let children = self.children.iter().map(Element::height).sum::<u32>();
        children + self.gap * self.children.len().saturating_sub(1) as u32
    }
}

#[derive(Debug, Clone)]
pub struct Card<'a> {
    pub content: Stack<'a>,
    pub padding: u32,
}

impl Card<'_> {
    pub fn height(&self) -> u32 {
        self.padding * 2
            + self
                .content
                .height
                .unwrap_or_else(|| self.content.content_height())
    }
}

#[derive(Debug, Clone)]
pub enum Block<'a> {
    Section(TextCell<'a>),
    Card(Card<'a>),
}

impl Block<'_> {
    pub fn height(&self) -> u32 {
        match self {
            Self::Section(_) => PANEL_GEOMETRY.section_height,
            Self::Card(card) => card.height(),
        }
    }
}

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
            value: memory.value(),
            subtitle: memory.subtitle(),
            secondary_label: MemoryPanel::SECONDARY_LABEL,
            secondary_value: memory.secondary_value(),
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
                .map(|row| (row.name, row.value))
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
                temperature.value(),
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
                fan.value(),
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
            value: cpu.value(),
            subtitle: String::new(),
            secondary_label: "",
            secondary_value: String::new(),
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
            processes: cpu.processes().map(|row| (row.name, row.value)).collect(),
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

#[derive(Debug, Clone, Copy)]
pub struct PanelGeometry {
    pub padding: u32,
    pub column_spacing: u32,
    pub section_height: u32,
    pub min_height: u32,
}

pub const PANEL_GEOMETRY: PanelGeometry = PanelGeometry {
    padding: 10,
    column_spacing: 8,
    section_height: 14,
    min_height: 220,
};

#[derive(Debug, Clone, Copy)]
pub struct MetricCardGeometry {
    pub spacing: u32,
    pub process_spacing: u32,
    pub header_height: u32,
    pub secondary_row_height: u32,
    pub process_row_height: u32,
    pub progress_height: u32,
    pub graph_height: u32,
}

pub const METRIC_CARD_GEOMETRY: MetricCardGeometry = MetricCardGeometry {
    spacing: 5,
    process_spacing: 2,
    header_height: 21,
    secondary_row_height: 15,
    process_row_height: 17,
    progress_height: 6,
    graph_height: 28,
};

#[derive(Debug, Clone, Copy)]
pub struct SmallGraphCardGeometry {
    pub spacing: u32,
    pub header_height: u32,
    pub graph_height: u32,
}

pub const SMALL_GRAPH_CARD_GEOMETRY: SmallGraphCardGeometry = SmallGraphCardGeometry {
    spacing: 4,
    header_height: 17,
    graph_height: 24,
};

#[derive(Debug, Clone, Copy)]
pub struct GpuCardGeometry {
    pub spacing: u32,
    pub header_height: u32,
    pub metric_row_height: u32,
    pub graph_height: u32,
    pub status_row_height: u32,
}

pub const GPU_CARD_GEOMETRY: GpuCardGeometry = GpuCardGeometry {
    spacing: 4,
    header_height: 17,
    metric_row_height: 17,
    graph_height: 22,
    status_row_height: 15,
};

#[derive(Debug, Clone, Copy)]
pub struct IoProcessGeometry {
    pub spacing: u32,
    pub row_height: u32,
    pub slots: u32,
}

pub const IO_PROCESS_GEOMETRY: IoProcessGeometry = IoProcessGeometry {
    spacing: 2,
    row_height: 17,
    slots: 3,
};

impl IoProcessGeometry {
    pub const fn height(self) -> u32 {
        self.slots * self.row_height + self.slots.saturating_sub(1) * self.spacing
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DiskCardGeometry {
    pub spacing: u32,
    pub header_height: u32,
    pub graph_height: u32,
}

pub const DISK_CARD_GEOMETRY: DiskCardGeometry = DiskCardGeometry {
    spacing: 4,
    header_height: 17,
    graph_height: 24,
};

#[derive(Debug, Clone, Copy)]
pub struct NetworkCardGeometry {
    pub spacing: u32,
    pub header_height: u32,
    pub value_row_height: u32,
    pub graph_height: u32,
}

pub const NETWORK_CARD_GEOMETRY: NetworkCardGeometry = NetworkCardGeometry {
    spacing: 4,
    header_height: 17,
    value_row_height: 17,
    graph_height: 22,
};

const CARD_PADDING: u32 = 9;
static EMPTY_GRAPH_VALUES: VecDeque<f64> = VecDeque::new();

fn section(title: &'static str) -> Block<'static> {
    Block::Section(TextCell::borrowed(title, 12, Tone::Muted).bold())
}

struct MetricCardParams<'a> {
    title: &'static str,
    value: String,
    subtitle: String,
    secondary_label: &'static str,
    secondary_value: String,
    progress: f32,
    graph: GraphSpec<'a>,
    processes: Vec<(&'a str, String)>,
}

fn metric_card<'a>(params: MetricCardParams<'a>) -> Block<'a> {
    let MetricCardParams {
        title,
        value,
        subtitle,
        secondary_label,
        secondary_value,
        progress,
        graph,
        processes,
    } = params;
    let mut header_cells = vec![
        TextCell::borrowed(title, 12, Tone::Muted)
            .bold()
            .grow()
            .clip(),
        TextCell::owned(value, 16, Tone::Foreground).bold(),
    ];
    if !subtitle.is_empty() {
        header_cells.push(TextCell::owned(subtitle, 11, Tone::Muted));
    }

    let process_children = processes
        .into_iter()
        .map(|(name, value)| {
            Element::Row(TextRow {
                cells: vec![
                    TextCell::borrowed(name, 12, Tone::Foreground).grow().clip(),
                    TextCell::owned(value, 12, Tone::Muted),
                ],
                height: METRIC_CARD_GEOMETRY.process_row_height,
                gap: 5,
            })
        })
        .collect::<Vec<_>>();

    card(Stack {
        children: vec![
            Element::Row(TextRow {
                cells: header_cells,
                height: METRIC_CARD_GEOMETRY.header_height,
                gap: 4,
            }),
            Element::Row(TextRow {
                cells: vec![
                    TextCell::borrowed(secondary_label, 11, Tone::Muted)
                        .bold()
                        .static_content()
                        .grow(),
                    TextCell::owned(secondary_value, 11, Tone::Muted),
                ],
                height: METRIC_CARD_GEOMETRY.secondary_row_height,
                gap: 4,
            }),
            Element::Progress(ProgressSpec {
                value: progress.clamp(0.0, 1.0),
                tone: graph.line,
                height: METRIC_CARD_GEOMETRY.progress_height,
            }),
            Element::Graph(graph),
            Element::Stack(Stack {
                children: process_children,
                gap: METRIC_CARD_GEOMETRY.process_spacing,
                height: None,
            }),
        ],
        gap: METRIC_CARD_GEOMETRY.spacing,
        height: None,
    })
}

fn gpu_card<'a>(gpu: cclover_presentation::GpuPanel<'a>, capacity: usize) -> Block<'a> {
    let graph = |values, min, max, line, alpha| GraphSpec {
        values,
        min,
        max,
        auto_scale: false,
        line,
        fill_alpha: alpha,
        capacity,
        height: GPU_CARD_GEOMETRY.graph_height,
    };
    card(Stack {
        children: vec![
            Element::Row(TextRow {
                cells: vec![
                    TextCell::borrowed(gpu.name(), 13, Tone::Foreground)
                        .bold()
                        .static_content()
                        .grow()
                        .clip(),
                ],
                height: GPU_CARD_GEOMETRY.header_height,
                gap: 0,
            }),
            metric_row(
                "UTILIZATION",
                gpu.utilization_value(),
                GPU_CARD_GEOMETRY.metric_row_height,
            ),
            Element::Graph(graph(
                gpu.utilization_history().unwrap_or(&EMPTY_GRAPH_VALUES),
                0.0,
                100.0,
                Tone::Accent,
                0.14,
            )),
            metric_row(
                "MEMORY",
                gpu.memory_value(),
                GPU_CARD_GEOMETRY.metric_row_height,
            ),
            Element::Graph(graph(
                gpu.memory_history().unwrap_or(&EMPTY_GRAPH_VALUES),
                0.0,
                gpu.memory_graph_max(),
                Tone::Green,
                0.13,
            )),
            metric_row(
                "TEMPERATURE",
                gpu.temperature_value(),
                GPU_CARD_GEOMETRY.metric_row_height,
            ),
            Element::Graph(graph(
                gpu.temperature_history().unwrap_or(&EMPTY_GRAPH_VALUES),
                20.0,
                100.0,
                Tone::Red,
                0.13,
            )),
            status_row("POWER", gpu.power_value()),
            status_row("CORE CLOCK", gpu.core_clock_value()),
            status_row("FAN", gpu.fan_value()),
        ],
        gap: GPU_CARD_GEOMETRY.spacing,
        height: None,
    })
}

fn metric_row<'a>(label: &'static str, value: String, height: u32) -> Element<'a> {
    Element::Row(TextRow {
        cells: vec![
            TextCell::borrowed(label, 10, Tone::Muted)
                .bold()
                .static_content()
                .grow(),
            TextCell::owned(value, 12, Tone::Foreground),
        ],
        height,
        gap: 0,
    })
}

fn status_row<'a>(label: &'static str, value: String) -> Element<'a> {
    Element::Row(TextRow {
        cells: vec![
            TextCell::borrowed(label, 10, Tone::Muted)
                .bold()
                .static_content()
                .grow(),
            TextCell::owned(value, 11, Tone::Muted),
        ],
        height: GPU_CARD_GEOMETRY.status_row_height,
        gap: 0,
    })
}

fn small_graph_card<'a>(name: &'a str, value: String, graph: GraphSpec<'a>) -> Block<'a> {
    card(Stack {
        children: vec![
            Element::Row(TextRow {
                cells: vec![
                    TextCell::borrowed(name, 13, Tone::Foreground)
                        .bold()
                        .grow()
                        .clip(),
                    TextCell::owned(value, 12, Tone::Foreground),
                ],
                height: SMALL_GRAPH_CARD_GEOMETRY.header_height,
                gap: 0,
            }),
            Element::Graph(graph),
        ],
        gap: SMALL_GRAPH_CARD_GEOMETRY.spacing,
        height: None,
    })
}

fn disk_card<'a>(disk: cclover_presentation::DiskPanel<'a>, capacity: usize) -> Block<'a> {
    let process_rows_visible = disk.process_rows_visible();
    let process_rows = io_process_rows(
        disk.processes()
            .map(|row| (row.name, row.pid, row.first_value, row.second_value)),
        "R",
        "W",
        Tone::Green,
        Tone::Orange,
    );
    let mut children = vec![
        Element::Row(TextRow {
            cells: vec![
                TextCell::owned(disk.name(), 13, Tone::Foreground)
                    .bold()
                    .grow()
                    .clip(),
                TextCell::owned(disk.value(), 12, Tone::Foreground),
            ],
            height: DISK_CARD_GEOMETRY.header_height,
            gap: 0,
        }),
        Element::Graph(GraphSpec {
            values: disk.history().unwrap_or(&EMPTY_GRAPH_VALUES),
            min: 0.0,
            max: 1.0,
            auto_scale: true,
            line: Tone::Orange,
            fill_alpha: 0.13,
            capacity,
            height: DISK_CARD_GEOMETRY.graph_height,
        }),
    ];
    if process_rows_visible {
        children.push(Element::Stack(process_rows));
    }
    card(Stack {
        children,
        gap: DISK_CARD_GEOMETRY.spacing,
        height: None,
    })
}

fn network_card<'a>(network: cclover_presentation::NetworkPanel<'a>, capacity: usize) -> Block<'a> {
    let (down, up) = network
        .history()
        .map(|history| (&history.down, &history.up))
        .unwrap_or((&EMPTY_GRAPH_VALUES, &EMPTY_GRAPH_VALUES));
    let process_rows_visible = network.process_rows_visible();
    let process_rows = io_process_rows(
        network
            .processes()
            .map(|row| (row.name, row.pid, row.first_value, row.second_value)),
        "↓",
        "↑",
        Tone::Green,
        Tone::Orange,
    );
    let mut children = vec![
        Element::Row(TextRow {
            cells: vec![
                TextCell::borrowed(network.name(), 13, Tone::Foreground)
                    .bold()
                    .static_content()
                    .grow()
                    .clip(),
            ],
            height: NETWORK_CARD_GEOMETRY.header_height,
            gap: 0,
        }),
        value_row("↓", network.down_value(), Tone::Green),
        Element::Graph(GraphSpec {
            values: down,
            min: 0.0,
            max: 1.0,
            auto_scale: true,
            line: Tone::Green,
            fill_alpha: 0.125,
            capacity,
            height: NETWORK_CARD_GEOMETRY.graph_height,
        }),
        value_row("↑", network.up_value(), Tone::Orange),
        Element::Graph(GraphSpec {
            values: up,
            min: 0.0,
            max: 1.0,
            auto_scale: true,
            line: Tone::Orange,
            fill_alpha: 0.125,
            capacity,
            height: NETWORK_CARD_GEOMETRY.graph_height,
        }),
    ];
    if process_rows_visible {
        children.push(Element::Stack(process_rows));
    }
    card(Stack {
        children,
        gap: NETWORK_CARD_GEOMETRY.spacing,
        height: None,
    })
}

fn value_row<'a>(label: &'static str, value: String, tone: Tone) -> Element<'a> {
    Element::Row(TextRow {
        cells: vec![
            TextCell::borrowed(label, 13, tone).static_content(),
            TextCell::owned(String::new(), 1, tone).grow(),
            TextCell::owned(value, 11, tone).bold(),
        ],
        height: NETWORK_CARD_GEOMETRY.value_row_height,
        gap: 4,
    })
}

fn io_process_rows<'a>(
    rows: impl Iterator<Item = (Option<&'a str>, u32, String, String)>,
    first_label: &'static str,
    second_label: &'static str,
    first_tone: Tone,
    second_tone: Tone,
) -> Stack<'a> {
    let mut children = Vec::new();
    for (name, pid, first, second) in rows {
        children.push(Element::Row(TextRow {
            cells: vec![
                TextCell::borrowed(name.unwrap_or("process"), 10, Tone::Foreground)
                    .grow()
                    .clip(),
                TextCell::owned(format!("·{pid}"), 9, Tone::Muted),
                TextCell::owned(format!("{first_label}{first}"), 9, first_tone),
                TextCell::owned(format!("{second_label}{second}"), 9, second_tone),
            ],
            height: IO_PROCESS_GEOMETRY.row_height,
            gap: 3,
        }));
    }
    Stack {
        children,
        gap: IO_PROCESS_GEOMETRY.spacing,
        height: Some(IO_PROCESS_GEOMETRY.height()),
    }
}

fn card(content: Stack<'_>) -> Block<'_> {
    Block::Card(Card {
        content,
        padding: CARD_PADDING,
    })
}

fn column_height(blocks: &[Block<'_>]) -> u32 {
    blocks.iter().map(Block::height).sum::<u32>()
        + PANEL_GEOMETRY.column_spacing * blocks.len().saturating_sub(1) as u32
}

#[cfg(test)]
mod tests {
    use cclover_core::model::{
        Collection, DiskMetadata, DiskSnapshot, MonitorState, NetworkSnapshot, TemperatureSnapshot,
    };

    use super::*;

    #[test]
    fn dashboard_tree_drives_dynamic_height() {
        let empty = MonitorState::default();
        let empty_ui = DashboardUi::new(Dashboard::new(&empty));
        assert_eq!(empty_ui.left.len(), 4);
        assert!(matches!(
            empty_ui.left.last(),
            Some(Block::Section(section)) if section.text.as_ref() == FAN_SECTION
        ));
        assert_eq!(empty_ui.right.len(), 3);

        let mut populated = MonitorState::default();
        populated.snapshot.temperatures = Collection::available(vec![TemperatureSnapshot {
            id: "cpu-temperature".to_owned(),
            name: "CPU".to_owned(),
            celsius: 50.0,
        }]);
        let populated_ui = DashboardUi::new(Dashboard::new(&populated));
        assert_eq!(populated_ui.left.len(), 5);
        assert!(populated_ui.height() > empty_ui.height());
    }

    #[test]
    fn io_top_regions_are_fixed_when_observable_and_omitted_when_unavailable() {
        let mut state = MonitorState::default();
        state.snapshot.disks = Collection::available(vec![DiskSnapshot {
            id: cclover_core::model::DiskId::from_opaque_key("disk-a"),
            metadata: DiskMetadata {
                system_label: "disk-a".into(),
                associated_labels: Vec::new(),
            },
            bytes_per_sec: 0.0,
        }]);
        state.snapshot.networks = Collection::available(vec![NetworkSnapshot {
            id: cclover_core::model::NetworkId::from_opaque_key("network-a"),
            name: "network-a".into(),
            down_bytes_per_sec: 0.0,
            up_bytes_per_sec: 0.0,
        }]);

        let dashboard = Dashboard::new(&state);
        let unavailable_disk_height = disk_card(dashboard.disk(0).unwrap(), 1).height();
        let unavailable_network_height = network_card(dashboard.network(0).unwrap(), 1).height();

        state.snapshot.process_disk_io = Collection::available(Vec::new());
        state.snapshot.process_network_io = Collection::available(Vec::new());
        let dashboard = Dashboard::new(&state);
        let available_disk_height = disk_card(dashboard.disk(0).unwrap(), 1).height();
        let available_network_height = network_card(dashboard.network(0).unwrap(), 1).height();

        assert_eq!(
            available_disk_height - unavailable_disk_height,
            IO_PROCESS_GEOMETRY.height() + DISK_CARD_GEOMETRY.spacing
        );
        assert_eq!(
            available_network_height - unavailable_network_height,
            IO_PROCESS_GEOMETRY.height() + NETWORK_CARD_GEOMETRY.spacing
        );
    }

    #[test]
    fn tree_owns_visual_semantics_not_renderer_types() {
        let state = MonitorState::default();
        let ui = DashboardUi::new(Dashboard::new(&state));
        let Block::Card(memory) = &ui.left[0] else {
            panic!("memory must be a card");
        };
        let Element::Row(header) = &memory.content.children[0] else {
            panic!("memory card must start with a row");
        };
        assert_eq!(header.cells[0].text, "MEMORY");
        assert_eq!(header.cells[0].tone, Tone::Muted);
        assert!(header.cells[0].grow);
        assert_eq!(Tone::Accent.rgba().b, 0xff);
    }
}
