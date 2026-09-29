mod graph;
mod layout;

use std::collections::VecDeque;

use graph::Graph;
use iced::border;
use iced::font::Weight;
use iced::widget::text::Wrapping;
use iced::widget::{Column, Space, canvas, column, container, progress_bar, row, text};
use iced::{Alignment, Border, Color, Element, Fill, Font, Theme};
use layout::{
    CARD_FRAME_GEOMETRY, DISK_CARD_GEOMETRY, GPU_MEMORY_CARD_GEOMETRY, IO_PROCESS_GEOMETRY,
    METRIC_CARD_GEOMETRY, NETWORK_CARD_GEOMETRY, PANEL_GEOMETRY, PanelBlock,
    SMALL_GRAPH_CARD_GEOMETRY,
};

use cclover_presentation::{CpuPanel, Dashboard, IoProcessRow, MemoryPanel, ProcessRow};

pub use layout::{INITIAL_PANEL_HEIGHT, PANEL_WIDTH, PanelLayout};

pub struct PanelState {
    layout: PanelLayout,
    surface_height: u32,
}

impl PanelState {
    pub fn new(dashboard: Dashboard<'_>) -> Self {
        let layout = PanelLayout::new(dashboard);
        Self {
            layout,
            surface_height: INITIAL_PANEL_HEIGHT,
        }
    }

    pub fn update(&mut self, dashboard: Dashboard<'_>) -> Option<u32> {
        let layout = PanelLayout::new(dashboard);
        let next_height = layout.height();
        self.layout = layout;
        if next_height == self.surface_height {
            return None;
        }

        self.surface_height = next_height;
        Some(next_height)
    }

    pub fn surface_height(&self) -> u32 {
        self.surface_height
    }

    pub fn view<'a, Message: 'a>(&'a self, dashboard: Dashboard<'a>) -> Element<'a, Message> {
        view(dashboard, &self.layout)
    }
}

const BG: Color = Color::from_rgba8(0x0b, 0x10, 0x20, 0.85);
const CARD: Color = Color::from_rgba8(0x15, 0x1b, 0x2d, 0.80);
const BORDER: Color = Color::from_rgb8(0x33, 0x41, 0x5f);
const FG: Color = Color::from_rgb8(0xf3, 0xf6, 0xff);
const MUTED: Color = Color::from_rgb8(0x8f, 0x9b, 0xb0);
const ACCENT: Color = Color::from_rgb8(0x7c, 0x9c, 0xff);
const GREEN: Color = Color::from_rgb8(0x52, 0xe0, 0xc4);
const ORANGE: Color = Color::from_rgb8(0xff, 0xb8, 0x6b);
const RED: Color = Color::from_rgb8(0xff, 0x7e, 0x9b);
#[cfg(all(not(target_os = "windows"), not(target_arch = "wasm32")))]
const MONO: Font = Font::with_name("Inconsolata");
#[cfg(any(target_os = "windows", target_arch = "wasm32"))]
const MONO: Font = Font::with_name("Fira Sans");

const MONO_BOLD: Font = Font {
    weight: Weight::Bold,
    ..MONO
};
static EMPTY_GRAPH_VALUES: VecDeque<f64> = VecDeque::new();

pub fn view<'a, Message>(dashboard: Dashboard<'a>, layout: &PanelLayout) -> Element<'a, Message>
where
    Message: 'a,
{
    let capacity = dashboard.history_capacity();
    let left = panel_column(dashboard, layout.left_blocks(), capacity);
    let right = panel_column(dashboard, layout.right_blocks(), capacity);

    let body = row![left, right]
        .spacing(PANEL_GEOMETRY.column_spacing)
        .width(Fill)
        .align_y(Alignment::Start);

    container(body)
        .padding(PANEL_GEOMETRY.padding as u16)
        .width(Fill)
        .style(|_| container::Style {
            background: Some(BG.into()),
            border: Border {
                color: BORDER,
                width: 1.0,
                radius: 16.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

fn panel_column<'a, Message>(
    dashboard: Dashboard<'a>,
    blocks: impl IntoIterator<Item = PanelBlock>,
    capacity: usize,
) -> Column<'a, Message>
where
    Message: 'a,
{
    let mut column = column![].spacing(PANEL_GEOMETRY.column_spacing).width(Fill);
    for block in blocks {
        column = column.push(block_view(dashboard, block, capacity));
    }
    column
}

fn block_view<'a, Message>(
    dashboard: Dashboard<'a>,
    block: PanelBlock,
    capacity: usize,
) -> Element<'a, Message>
where
    Message: 'a,
{
    match block {
        PanelBlock::Memory { process_count } => {
            let memory = dashboard.memory();
            metric_card(MetricCardParams {
                title: MemoryPanel::TITLE,
                value: memory.value(),
                subtitle: memory.subtitle(),
                secondary_label: MemoryPanel::SECONDARY_LABEL,
                secondary_value: memory.secondary_value(),
                progress: memory.fraction(),
                graph: GraphParams {
                    values: memory.graph_values(),
                    min: 0.0,
                    max: memory.graph_max(),
                    auto_scale: false,
                    color: GREEN,
                    fill_color: Color::from_rgba8(0x52, 0xe0, 0xc4, 0.13),
                    capacity,
                },
                processes: memory.processes().collect(),
                process_count,
            })
        }
        PanelBlock::Cpu { process_count } => {
            let cpu = dashboard.cpu();
            metric_card(MetricCardParams {
                title: CpuPanel::TITLE,
                value: cpu.value(),
                subtitle: String::new(),
                secondary_label: "",
                secondary_value: String::new(),
                progress: cpu.fraction(),
                graph: GraphParams {
                    values: cpu.graph_values(),
                    min: 0.0,
                    max: 100.0,
                    auto_scale: false,
                    color: ACCENT,
                    fill_color: Color::from_rgba8(0x7c, 0x9c, 0xff, 0.14),
                    capacity,
                },
                processes: cpu.processes().collect(),
                process_count,
            })
        }
        PanelBlock::Section(section) => section_label(section.title()),
        PanelBlock::GpuMemory(index) => {
            let gpu = dashboard
                .gpu_memory(index)
                .expect("panel layout must match dashboard GPU memory entries");
            gpu_memory_card(
                gpu.name(),
                gpu.value(),
                gpu.percent(),
                gpu.fraction(),
                gpu.history().unwrap_or(&EMPTY_GRAPH_VALUES),
                gpu.graph_max(),
                capacity,
            )
        }
        PanelBlock::Temperature(index) => {
            let temperature = dashboard
                .temperature(index)
                .expect("panel layout must match dashboard temperature entries");
            small_graph_card(SmallGraphCardParams {
                name: temperature.name(),
                value: temperature.value(),
                graph: GraphParams {
                    values: temperature.history().unwrap_or(&EMPTY_GRAPH_VALUES),
                    min: 20.0,
                    max: 100.0,
                    auto_scale: false,
                    color: RED,
                    fill_color: Color::from_rgba8(0xff, 0x7e, 0x9b, 0.13),
                    capacity,
                },
            })
        }
        PanelBlock::Disk(index) => {
            let disk = dashboard
                .disk(index)
                .expect("panel layout must match dashboard disk entries");
            disk_card(IoGraphCardParams {
                name: disk.name(),
                value: disk.value(),
                graph: GraphParams {
                    values: disk.history().unwrap_or(&EMPTY_GRAPH_VALUES),
                    min: 0.0,
                    max: 1.0,
                    auto_scale: true,
                    color: ORANGE,
                    fill_color: Color::from_rgba8(0xff, 0xb8, 0x6b, 0.13),
                    capacity,
                },
                processes: disk.processes().collect(),
                process_unavailable_value: disk.process_unavailable_value(),
            })
        }
        PanelBlock::Network(index) => {
            let network = dashboard
                .network(index)
                .expect("panel layout must match dashboard network entries");
            let (down_history, up_history) = network
                .history()
                .map(|history| (&history.down, &history.up))
                .unwrap_or((&EMPTY_GRAPH_VALUES, &EMPTY_GRAPH_VALUES));
            network_card(NetworkCardParams {
                name: network.name(),
                down_value: network.down_value(),
                up_value: network.up_value(),
                down_values: down_history,
                up_values: up_history,
                capacity,
                processes: network.processes().collect(),
                process_unavailable_value: network.process_unavailable_value(),
            })
        }
    }
}

struct GraphParams<'a> {
    values: &'a VecDeque<f64>,
    min: f64,
    max: f64,
    auto_scale: bool,
    color: Color,
    fill_color: Color,
    capacity: usize,
}

struct MetricCardParams<'a> {
    title: &'a str,
    value: String,
    subtitle: String,
    secondary_label: &'a str,
    secondary_value: String,
    progress: f32,
    graph: GraphParams<'a>,
    processes: Vec<ProcessRow<'a>>,
    process_count: usize,
}

fn metric_card<'a, Message>(params: MetricCardParams<'a>) -> Element<'a, Message>
where
    Message: 'a,
{
    let MetricCardParams {
        title,
        value,
        subtitle,
        secondary_label,
        secondary_value,
        progress,
        graph,
        processes,
        process_count,
    } = params;
    let mut header = row![
        bold_label_owned(title.to_owned(), 12, MUTED).width(Fill),
        bold_label_owned(value, 16, FG),
    ];
    if !subtitle.is_empty() {
        header = header.push(label_owned(subtitle, 11, MUTED));
    }
    let header = header
        .spacing(4)
        .height(METRIC_CARD_GEOMETRY.header_height)
        .align_y(Alignment::Center);

    let secondary = row![
        bold_label_owned(secondary_label.to_owned(), 11, MUTED).width(Fill),
        label_owned(secondary_value, 11, MUTED),
    ]
    .spacing(4)
    .height(METRIC_CARD_GEOMETRY.secondary_row_height)
    .align_y(Alignment::Center);

    let mut content = column![header].spacing(METRIC_CARD_GEOMETRY.spacing);
    content = content.push(secondary);
    content = content.push(
        progress_bar(0.0..=1.0, progress.clamp(0.0, 1.0))
            .girth(METRIC_CARD_GEOMETRY.progress_height)
            .style(move |_| iced::widget::progress_bar::Style {
                background: BORDER.into(),
                bar: graph.color.into(),
                border: border::rounded(3),
            }),
    );

    let graph = Graph::new(graph.values, graph.capacity, graph.color, graph.fill_color)
        .range(graph.min, graph.max)
        .auto_scale(graph.auto_scale);
    content = content.push(
        canvas(graph)
            .width(Fill)
            .height(METRIC_CARD_GEOMETRY.graph_height),
    );

    let mut process_body = column![]
        .spacing(METRIC_CARD_GEOMETRY.process_spacing)
        .width(Fill);
    for process in processes {
        process_body = process_body.push(process_row(process));
    }
    content = content.push(process_body);

    card(content, METRIC_CARD_GEOMETRY.height(process_count as u32))
}

struct SmallGraphCardParams<'a> {
    name: &'a str,
    value: String,
    graph: GraphParams<'a>,
}

struct IoGraphCardParams<'a> {
    name: &'a str,
    value: String,
    graph: GraphParams<'a>,
    processes: Vec<IoProcessRow<'a>>,
    process_unavailable_value: Option<&'static str>,
}

struct NetworkCardParams<'a> {
    name: &'a str,
    down_value: String,
    up_value: String,
    down_values: &'a VecDeque<f64>,
    up_values: &'a VecDeque<f64>,
    capacity: usize,
    processes: Vec<IoProcessRow<'a>>,
    process_unavailable_value: Option<&'static str>,
}

fn gpu_memory_card<'a, Message>(
    name: &'a str,
    value: String,
    percent: String,
    fraction: f32,
    history: &'a VecDeque<f64>,
    graph_max: f64,
    capacity: usize,
) -> Element<'a, Message>
where
    Message: 'a,
{
    let header = container(
        bold_label(name, 13, FG)
            .wrapping(Wrapping::None)
            .width(Fill),
    )
    .height(GPU_MEMORY_CARD_GEOMETRY.header_height)
    .width(Fill)
    .clip(true);
    let values = row![
        label_owned(value, 12, FG).width(Fill),
        bold_label_owned(percent, 11, MUTED),
    ]
    .height(GPU_MEMORY_CARD_GEOMETRY.value_row_height)
    .align_y(Alignment::Center);
    let progress = progress_bar(0.0..=1.0, fraction.clamp(0.0, 1.0))
        .girth(GPU_MEMORY_CARD_GEOMETRY.progress_height)
        .style(|_| iced::widget::progress_bar::Style {
            background: BORDER.into(),
            bar: GREEN.into(),
            border: border::rounded(3),
        });
    let graph = Graph::new(
        history,
        capacity,
        GREEN,
        Color::from_rgba8(0x52, 0xe0, 0xc4, 0.13),
    )
    .range(0.0, graph_max)
    .auto_scale(false);

    card(
        column![
            header,
            values,
            progress,
            canvas(graph)
                .width(Fill)
                .height(GPU_MEMORY_CARD_GEOMETRY.graph_height)
        ]
        .spacing(GPU_MEMORY_CARD_GEOMETRY.spacing),
        GPU_MEMORY_CARD_GEOMETRY.height(),
    )
}

fn small_graph_card<'a, Message>(params: SmallGraphCardParams<'a>) -> Element<'a, Message>
where
    Message: 'a,
{
    let SmallGraphCardParams { name, value, graph } = params;
    let header = row![
        bold_label(name, 13, FG)
            .wrapping(Wrapping::None)
            .width(Fill),
        label_owned(value, 12, FG)
    ]
    .height(SMALL_GRAPH_CARD_GEOMETRY.header_height)
    .align_y(Alignment::Center);
    let graph = Graph::new(graph.values, graph.capacity, graph.color, graph.fill_color)
        .range(graph.min, graph.max)
        .auto_scale(graph.auto_scale);
    card(
        column![
            header,
            canvas(graph)
                .width(Fill)
                .height(SMALL_GRAPH_CARD_GEOMETRY.graph_height)
        ]
        .spacing(SMALL_GRAPH_CARD_GEOMETRY.spacing),
        SMALL_GRAPH_CARD_GEOMETRY.height(),
    )
}

fn disk_card<'a, Message>(params: IoGraphCardParams<'a>) -> Element<'a, Message>
where
    Message: 'a,
{
    let IoGraphCardParams {
        name,
        value,
        graph,
        processes,
        process_unavailable_value,
    } = params;
    let header = row![
        bold_label(name, 13, FG)
            .wrapping(Wrapping::None)
            .width(Fill),
        label_owned(value, 12, FG)
    ]
    .height(DISK_CARD_GEOMETRY.header_height)
    .align_y(Alignment::Center);
    let graph = Graph::new(graph.values, graph.capacity, graph.color, graph.fill_color)
        .range(graph.min, graph.max)
        .auto_scale(graph.auto_scale);

    card(
        column![
            header,
            canvas(graph)
                .width(Fill)
                .height(DISK_CARD_GEOMETRY.graph_height),
            io_process_body(
                processes,
                process_unavailable_value,
                "R",
                "W",
                GREEN,
                ORANGE,
            ),
        ]
        .spacing(DISK_CARD_GEOMETRY.spacing),
        DISK_CARD_GEOMETRY.height(),
    )
}

fn network_card<'a, Message>(params: NetworkCardParams<'a>) -> Element<'a, Message>
where
    Message: 'a,
{
    let NetworkCardParams {
        name,
        down_value,
        up_value,
        down_values,
        up_values,
        capacity,
        processes,
        process_unavailable_value,
    } = params;
    let down_graph = Graph::new(
        down_values,
        capacity,
        GREEN,
        Color::from_rgba8(0x52, 0xe0, 0xc4, 0.125),
    )
    .auto_scale(true);
    let up_graph = Graph::new(
        up_values,
        capacity,
        ORANGE,
        Color::from_rgba8(0xff, 0xb8, 0x6b, 0.125),
    )
    .auto_scale(true);

    card(
        column![
            container(
                bold_label(name, 13, FG)
                    .wrapping(Wrapping::None)
                    .width(Fill)
            )
            .height(NETWORK_CARD_GEOMETRY.header_height)
            .width(Fill)
            .clip(true),
            graph_value_row("↓", down_value, GREEN),
            canvas(down_graph)
                .width(Fill)
                .height(NETWORK_CARD_GEOMETRY.graph_height),
            graph_value_row("↑", up_value, ORANGE),
            canvas(up_graph)
                .width(Fill)
                .height(NETWORK_CARD_GEOMETRY.graph_height),
            io_process_body(
                processes,
                process_unavailable_value,
                "↓",
                "↑",
                GREEN,
                ORANGE,
            ),
        ]
        .spacing(NETWORK_CARD_GEOMETRY.spacing),
        NETWORK_CARD_GEOMETRY.height(),
    )
}

fn io_process_body<'a, Message>(
    processes: Vec<IoProcessRow<'a>>,
    unavailable_value: Option<&'static str>,
    first_label: &'static str,
    second_label: &'static str,
    first_color: Color,
    second_color: Color,
) -> Column<'a, Message>
where
    Message: 'a,
{
    let mut body = column![]
        .spacing(IO_PROCESS_GEOMETRY.spacing)
        .height(IO_PROCESS_GEOMETRY.height())
        .width(Fill);
    if let Some(value) = unavailable_value {
        body = body.push(label(value, 10, MUTED));
    }
    for process in processes {
        body = body.push(io_process_row(
            process,
            first_label,
            second_label,
            first_color,
            second_color,
        ));
    }
    body
}

fn io_process_row<'a, Message>(
    process: IoProcessRow<'a>,
    first_label: &'static str,
    second_label: &'static str,
    first_color: Color,
    second_color: Color,
) -> Element<'a, Message>
where
    Message: 'a,
{
    let identity = row![
        label(process.name.unwrap_or("process"), 10, FG).wrapping(Wrapping::None),
        label_owned(format!("·{}", process.pid), 9, MUTED),
    ]
    .spacing(2)
    .align_y(Alignment::Center);
    row![
        container(identity).width(Fill).clip(true),
        label_owned(
            format!("{first_label}{}", process.first_value),
            9,
            first_color
        ),
        label_owned(
            format!("{second_label}{}", process.second_value),
            9,
            second_color
        ),
    ]
    .spacing(3)
    .height(IO_PROCESS_GEOMETRY.row_height)
    .align_y(Alignment::Center)
    .into()
}

fn process_row<'a, Message>(process: ProcessRow<'a>) -> Element<'a, Message>
where
    Message: 'a,
{
    row![
        container(
            label(process.name, 12, FG)
                .wrapping(Wrapping::None)
                .width(Fill)
        )
        .width(Fill)
        .clip(true),
        label_owned(process.value, 12, MUTED)
    ]
    .spacing(5)
    .height(METRIC_CARD_GEOMETRY.process_row_height)
    .align_y(Alignment::Center)
    .into()
}

fn graph_value_row<'a, Message>(
    label_text: &'a str,
    value: String,
    color: Color,
) -> Element<'a, Message>
where
    Message: 'a,
{
    row![
        label(label_text, 13, color),
        Space::new().width(Fill),
        bold_label_owned(value, 11, color),
    ]
    .spacing(4)
    .height(NETWORK_CARD_GEOMETRY.value_row_height)
    .align_y(Alignment::Center)
    .into()
}

fn section_label<'a, Message>(value: &'a str) -> Element<'a, Message>
where
    Message: 'a,
{
    container(bold_label(value, 12, MUTED))
        .height(PANEL_GEOMETRY.section_height)
        .width(Fill)
        .into()
}

fn card<'a, Message>(content: Column<'a, Message>, height: u32) -> Element<'a, Message>
where
    Message: 'a,
{
    container(content)
        .padding(CARD_FRAME_GEOMETRY.padding as u16)
        .height(height)
        .width(Fill)
        .style(|_| container::Style {
            background: Some(CARD.into()),
            border: Border {
                color: BORDER,
                width: 1.0,
                radius: 12.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

fn label<'a>(value: &'a str, size: u32, color: Color) -> iced::widget::Text<'a> {
    text(value).font(MONO).size(size).color(color)
}

fn label_owned(value: String, size: u32, color: Color) -> iced::widget::Text<'static> {
    text(value).font(MONO).size(size).color(color)
}

fn bold_label<'a>(value: &'a str, size: u32, color: Color) -> iced::widget::Text<'a> {
    text(value).font(MONO_BOLD).size(size).color(color)
}

fn bold_label_owned(value: String, size: u32, color: Color) -> iced::widget::Text<'static> {
    text(value).font(MONO_BOLD).size(size).color(color)
}

pub fn theme() -> Theme {
    Theme::Dark
}
