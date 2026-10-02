use std::collections::VecDeque;

use crate::{
    Block, Card, DISK_CARD_GEOMETRY, Element, GPU_CARD_GEOMETRY, GraphSpec, IO_PROCESS_GEOMETRY,
    METRIC_CARD_GEOMETRY, NETWORK_CARD_GEOMETRY, ProgressSpec, SMALL_GRAPH_CARD_GEOMETRY, Stack,
    TEXT_SLOT_GEOMETRY, TextCell, TextRow, Tone,
};

pub(crate) const CARD_PADDING: u32 = 9;
pub(crate) static EMPTY_GRAPH_VALUES: VecDeque<f64> = VecDeque::new();

pub(crate) fn section(title: &'static str) -> Block<'static> {
    Block::Section(TextCell::borrowed(title, 12, Tone::Muted).bold())
}

pub(crate) struct MetricCardParams<'a> {
    pub(crate) title: &'static str,
    pub(crate) value: String,
    pub(crate) subtitle: String,
    pub(crate) secondary_label: &'static str,
    pub(crate) secondary_value: String,
    pub(crate) progress: f32,
    pub(crate) graph: GraphSpec<'a>,
    pub(crate) processes: Vec<(&'a str, String)>,
}

pub(crate) fn metric_card<'a>(params: MetricCardParams<'a>) -> Block<'a> {
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
            .fill()
            .clip(),
        TextCell::owned(value, 16, Tone::Foreground)
            .bold()
            .fixed(TEXT_SLOT_GEOMETRY.metric_value),
    ];
    if !subtitle.is_empty() {
        header_cells.push(
            TextCell::owned(subtitle, 11, Tone::Muted).fixed(TEXT_SLOT_GEOMETRY.metric_subtitle),
        );
    }

    let process_children = processes
        .into_iter()
        .map(|(name, value)| {
            Element::Row(TextRow {
                cells: vec![
                    TextCell::borrowed(name, 12, Tone::Foreground).fill().clip(),
                    TextCell::owned(value, 12, Tone::Muted).fixed(TEXT_SLOT_GEOMETRY.process_value),
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
                        .fill(),
                    TextCell::owned(secondary_value, 11, Tone::Muted)
                        .fixed(TEXT_SLOT_GEOMETRY.secondary_value),
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

pub(crate) fn gpu_card<'a>(gpu: cclover_presentation::GpuPanel<'a>, capacity: usize) -> Block<'a> {
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
                        .fill()
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
                .fill(),
            TextCell::owned(value, 12, Tone::Foreground).fixed(TEXT_SLOT_GEOMETRY.gpu_value),
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
                .fill(),
            TextCell::owned(value, 11, Tone::Muted).fixed(TEXT_SLOT_GEOMETRY.gpu_value),
        ],
        height: GPU_CARD_GEOMETRY.status_row_height,
        gap: 0,
    })
}

pub(crate) fn small_graph_card<'a>(
    name: &'a str,
    value: String,
    graph: GraphSpec<'a>,
) -> Block<'a> {
    card(Stack {
        children: vec![
            Element::Row(TextRow {
                cells: vec![
                    TextCell::borrowed(name, 13, Tone::Foreground)
                        .bold()
                        .fill()
                        .clip(),
                    TextCell::owned(value, 12, Tone::Foreground)
                        .fixed(TEXT_SLOT_GEOMETRY.card_value),
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

pub(crate) fn disk_card<'a>(
    disk: cclover_presentation::DiskPanel<'a>,
    capacity: usize,
) -> Block<'a> {
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
                    .fill()
                    .clip(),
                TextCell::owned(disk.value(), 12, Tone::Foreground)
                    .fixed(TEXT_SLOT_GEOMETRY.card_value),
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

pub(crate) fn network_card<'a>(
    network: cclover_presentation::NetworkPanel<'a>,
    capacity: usize,
) -> Block<'a> {
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
                    .fill()
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
            TextCell::borrowed(label, 13, tone)
                .static_content()
                .fixed(14)
                .align_start(),
            TextCell::owned(String::new(), 1, tone).fill(),
            TextCell::owned(value, 11, tone)
                .bold()
                .fixed(TEXT_SLOT_GEOMETRY.network_value),
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
                    .fill()
                    .clip(),
                TextCell::owned(format!("·{pid}"), 9, Tone::Muted).fixed(TEXT_SLOT_GEOMETRY.io_pid),
                TextCell::owned(format!("{first_label}{first}"), 9, first_tone)
                    .fixed(TEXT_SLOT_GEOMETRY.io_value),
                TextCell::owned(format!("{second_label}{second}"), 9, second_tone)
                    .fixed(TEXT_SLOT_GEOMETRY.io_value),
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
