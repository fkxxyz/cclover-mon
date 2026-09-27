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
    CARD_PADDING, COLUMN_SPACING, GRAPH_VALUE_ROW_HEIGHT, METRIC_GRAPH_HEIGHT,
    METRIC_HEADER_HEIGHT, METRIC_SPACING, NETWORK_GRAPH_HEIGHT, PANEL_PADDING, PROCESS_ROW_HEIGHT,
    PROCESS_SPACING, PROGRESS_HEIGHT, PanelBlock, SECONDARY_ROW_HEIGHT, SECTION_HEIGHT,
    SMALL_GRAPH_HEIGHT, SMALL_HEADER_HEIGHT, SMALL_SPACING,
};

use crate::presentation::{CpuPanel, Dashboard, MemoryPanel, ProcessRow};

pub use layout::{INITIAL_PANEL_HEIGHT, PANEL_WIDTH, PanelLayout};

const BG: Color = Color::from_rgba8(0x0b, 0x10, 0x20, 0.85);
const CARD: Color = Color::from_rgba8(0x15, 0x1b, 0x2d, 0.80);
const BORDER: Color = Color::from_rgb8(0x33, 0x41, 0x5f);
const FG: Color = Color::from_rgb8(0xf3, 0xf6, 0xff);
const MUTED: Color = Color::from_rgb8(0x8f, 0x9b, 0xb0);
const ACCENT: Color = Color::from_rgb8(0x7c, 0x9c, 0xff);
const GREEN: Color = Color::from_rgb8(0x52, 0xe0, 0xc4);
const ORANGE: Color = Color::from_rgb8(0xff, 0xb8, 0x6b);
const RED: Color = Color::from_rgb8(0xff, 0x7e, 0x9b);
const MONO: Font = Font::with_name("Inconsolata");
const MONO_BOLD: Font = Font {
    weight: Weight::Bold,
    ..Font::with_name("Inconsolata")
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
        .spacing(COLUMN_SPACING)
        .width(Fill)
        .align_y(Alignment::Start);

    container(body)
        .padding(PANEL_PADDING as u16)
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
    let mut column = column![].spacing(COLUMN_SPACING).width(Fill);
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
        PanelBlock::Memory { .. } => {
            let memory = dashboard.memory();
            metric_card(
                MemoryPanel::TITLE,
                memory.value(),
                memory.subtitle(),
                MemoryPanel::SECONDARY_LABEL,
                memory.secondary_value(),
                Some(memory.fraction()),
                memory.graph_values(),
                0.0,
                memory.graph_max(),
                false,
                GREEN,
                Color::from_rgba8(0x52, 0xe0, 0xc4, 0.13),
                memory.processes(),
                capacity,
            )
        }
        PanelBlock::Cpu { .. } => {
            let cpu = dashboard.cpu();
            metric_card(
                CpuPanel::TITLE,
                cpu.value(),
                String::new(),
                "",
                String::new(),
                Some(cpu.fraction()),
                cpu.graph_values(),
                0.0,
                100.0,
                false,
                ACCENT,
                Color::from_rgba8(0x7c, 0x9c, 0xff, 0.14),
                cpu.processes(),
                capacity,
            )
        }
        PanelBlock::Section(section) => section_label(section.title()),
        PanelBlock::Temperature(index) => {
            let temperature = dashboard
                .temperature(index)
                .expect("panel layout must match dashboard temperature entries");
            small_graph_card(
                temperature.name(),
                temperature.value(),
                temperature.history().unwrap_or(&EMPTY_GRAPH_VALUES),
                20.0,
                100.0,
                false,
                RED,
                Color::from_rgba8(0xff, 0x7e, 0x9b, 0.13),
                capacity,
            )
        }
        PanelBlock::Disk(index) => {
            let disk = dashboard
                .disk(index)
                .expect("panel layout must match dashboard disk entries");
            small_graph_card(
                disk.name(),
                disk.value(),
                disk.history().unwrap_or(&EMPTY_GRAPH_VALUES),
                0.0,
                1.0,
                true,
                ORANGE,
                Color::from_rgba8(0xff, 0xb8, 0x6b, 0.13),
                capacity,
            )
        }
        PanelBlock::Network(index) => {
            let network = dashboard
                .network(index)
                .expect("panel layout must match dashboard network entries");
            let (down_history, up_history) = network
                .history()
                .map(|history| (&history.down, &history.up))
                .unwrap_or((&EMPTY_GRAPH_VALUES, &EMPTY_GRAPH_VALUES));
            network_card(
                network.name(),
                network.down_value(),
                network.up_value(),
                down_history,
                up_history,
                capacity,
            )
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn metric_card<'a, Message>(
    title: &'a str,
    value: String,
    subtitle: String,
    secondary_label: &'a str,
    secondary_value: String,
    progress: Option<f32>,
    graph_values: &'a VecDeque<f64>,
    graph_min: f64,
    graph_max: f64,
    auto_scale: bool,
    graph_color: Color,
    fill_color: Color,
    processes: impl IntoIterator<Item = ProcessRow<'a>>,
    capacity: usize,
) -> Element<'a, Message>
where
    Message: 'a,
{
    let mut header = row![
        bold_label_owned(title.to_owned(), 12, MUTED).width(Fill),
        bold_label_owned(value, 16, FG),
    ];
    if !subtitle.is_empty() {
        header = header.push(label_owned(subtitle, 11, MUTED));
    }
    let header = header
        .spacing(4)
        .height(METRIC_HEADER_HEIGHT)
        .align_y(Alignment::Center);

    let secondary = row![
        bold_label_owned(secondary_label.to_owned(), 11, MUTED).width(Fill),
        label_owned(secondary_value, 11, MUTED),
    ]
    .spacing(4)
    .height(SECONDARY_ROW_HEIGHT)
    .align_y(Alignment::Center);

    let mut content = column![header].spacing(METRIC_SPACING);
    content = content.push(secondary);
    if let Some(progress) = progress {
        content = content.push(
            progress_bar(0.0..=1.0, progress.clamp(0.0, 1.0))
                .girth(PROGRESS_HEIGHT)
                .style(move |_| iced::widget::progress_bar::Style {
                    background: BORDER.into(),
                    bar: graph_color.into(),
                    border: border::rounded(3),
                }),
        );
    }

    let graph = Graph::new(graph_values, capacity, graph_color, fill_color)
        .range(graph_min, graph_max)
        .auto_scale(auto_scale);
    content = content.push(canvas(graph).width(Fill).height(METRIC_GRAPH_HEIGHT));

    let mut process_body = column![].spacing(PROCESS_SPACING).width(Fill);
    for process in processes {
        process_body = process_body.push(process_row(process));
    }
    content = content.push(process_body);

    card(content)
}

#[allow(clippy::too_many_arguments)]
fn small_graph_card<'a, Message>(
    name: &'a str,
    value: String,
    values: &'a VecDeque<f64>,
    min: f64,
    max: f64,
    auto_scale: bool,
    graph_color: Color,
    fill_color: Color,
    capacity: usize,
) -> Element<'a, Message>
where
    Message: 'a,
{
    let header = row![
        bold_label(name, 13, FG)
            .wrapping(Wrapping::None)
            .width(Fill),
        label_owned(value, 12, FG)
    ]
    .height(SMALL_HEADER_HEIGHT)
    .align_y(Alignment::Center);
    let graph = Graph::new(values, capacity, graph_color, fill_color)
        .range(min, max)
        .auto_scale(auto_scale);
    card(
        column![header, canvas(graph).width(Fill).height(SMALL_GRAPH_HEIGHT)]
            .spacing(SMALL_SPACING),
    )
}

fn network_card<'a, Message>(
    name: &'a str,
    down_value: String,
    up_value: String,
    down_values: &'a VecDeque<f64>,
    up_values: &'a VecDeque<f64>,
    capacity: usize,
) -> Element<'a, Message>
where
    Message: 'a,
{
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
            .height(SMALL_HEADER_HEIGHT)
            .width(Fill)
            .clip(true),
            graph_value_row("↓", down_value, GREEN),
            canvas(down_graph).width(Fill).height(NETWORK_GRAPH_HEIGHT),
            graph_value_row("↑", up_value, ORANGE),
            canvas(up_graph).width(Fill).height(NETWORK_GRAPH_HEIGHT),
        ]
        .spacing(SMALL_SPACING),
    )
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
    .height(PROCESS_ROW_HEIGHT)
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
    .height(GRAPH_VALUE_ROW_HEIGHT)
    .align_y(Alignment::Center)
    .into()
}

fn section_label<'a, Message>(value: &'a str) -> Element<'a, Message>
where
    Message: 'a,
{
    container(bold_label(value, 12, MUTED))
        .height(SECTION_HEIGHT)
        .width(Fill)
        .into()
}

fn card<'a, Message>(content: Column<'a, Message>) -> Element<'a, Message>
where
    Message: 'a,
{
    container(content)
        .padding(CARD_PADDING as u16)
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
