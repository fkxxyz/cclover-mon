mod graph;

use std::collections::VecDeque;

use graph::Graph;
use iced::border;
use iced::font::Weight;
use iced::widget::text::Wrapping;
use iced::widget::{Column, Space, canvas, column, container, progress_bar, row, text};
use iced::{Alignment, Border, Color, Element, Fill, Font, Theme};

use crate::app::Message;
use crate::core::model::MonitorState;

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

const PANEL_PADDING: u32 = 10;
const COLUMN_SPACING: u32 = 8;
const CARD_PADDING: u32 = 9;
const METRIC_SPACING: u32 = 5;
const PROCESS_SPACING: u32 = 2;
const SMALL_SPACING: u32 = 4;
const METRIC_HEADER_HEIGHT: u32 = 21;
const SECONDARY_ROW_HEIGHT: u32 = 15;
const PROCESS_ROW_HEIGHT: u32 = 17;
const SMALL_HEADER_HEIGHT: u32 = 17;
const GRAPH_VALUE_ROW_HEIGHT: u32 = 17;
const SECTION_HEIGHT: u32 = 14;
const PROGRESS_HEIGHT: u32 = 6;
const METRIC_GRAPH_HEIGHT: u32 = 28;
const SMALL_GRAPH_HEIGHT: u32 = 24;
const NETWORK_GRAPH_HEIGHT: u32 = 22;

pub fn view(state: &MonitorState) -> Element<'_, Message> {
    let snapshot = &state.snapshot;
    let history = &state.history;
    let capacity = state.history_capacity.max(1);

    let memory_used = snapshot.memory.as_ref().map(|memory| memory.used_bytes);
    let memory_total = snapshot.memory.as_ref().map(|memory| memory.total_bytes);
    let swap_used = snapshot
        .memory
        .as_ref()
        .map(|memory| memory.swap_used_bytes);
    let swap_total = snapshot
        .memory
        .as_ref()
        .map(|memory| memory.swap_total_bytes);
    let memory_fraction = match (memory_used, memory_total) {
        (Some(used), Some(total)) if total > 0 => used as f32 / total as f32,
        _ => 0.0,
    };
    let memory_processes: Vec<(String, String)> = snapshot
        .top_memory
        .iter()
        .map(|process| (process.name.clone(), bytes(process.bytes)))
        .collect();

    let mut left = column![metric_card(
        "MEMORY",
        memory_used.map(bytes).unwrap_or_else(unavailable),
        memory_total
            .map(|total| format!("/ {}", bytes(total)))
            .unwrap_or_default(),
        "SWAP",
        match (swap_used, swap_total) {
            (Some(used), Some(total)) => format!("{} / {}", bytes(used), bytes(total)),
            _ => unavailable(),
        },
        Some(memory_fraction),
        &history.memory_used,
        0.0,
        memory_total.unwrap_or(1) as f64,
        false,
        GREEN,
        Color::from_rgba8(0x52, 0xe0, 0xc4, 0.13),
        memory_processes,
        capacity,
    )]
    .spacing(COLUMN_SPACING)
    .width(Fill);

    left = left.push(section_label("TEMPERATURE"));
    for temperature in &snapshot.temperatures {
        let values = history
            .temperatures
            .get(&temperature.name)
            .cloned()
            .unwrap_or_default();
        left = left.push(small_graph_card(
            &temperature.name,
            format!("{:.1}°C", temperature.celsius),
            &values,
            20.0,
            100.0,
            false,
            RED,
            Color::from_rgba8(0xff, 0x7e, 0x9b, 0.13),
            capacity,
        ));
    }

    let cpu_value = snapshot
        .cpu_percent
        .map(|value| format!("{value:.1}%"))
        .unwrap_or_else(unavailable);
    let cpu_fraction = snapshot.cpu_percent.unwrap_or(0.0) as f32 / 100.0;
    let cpu_processes: Vec<(String, String)> = snapshot
        .top_cpu
        .iter()
        .map(|process| (process.name.clone(), format!("{:.1}%", process.percent)))
        .collect();

    let mut right = column![metric_card(
        "CPU",
        cpu_value,
        String::new(),
        "",
        String::new(),
        Some(cpu_fraction),
        &history.cpu,
        0.0,
        100.0,
        false,
        ACCENT,
        Color::from_rgba8(0x7c, 0x9c, 0xff, 0.14),
        cpu_processes,
        capacity,
    )]
    .spacing(COLUMN_SPACING)
    .width(Fill);

    right = right.push(section_label("DISK I/O"));
    for disk in &snapshot.disks {
        let values = history.disks.get(&disk.name).cloned().unwrap_or_default();
        right = right.push(small_graph_card(
            &disk.name,
            rate(disk.bytes_per_sec),
            &values,
            0.0,
            1.0,
            true,
            ORANGE,
            Color::from_rgba8(0xff, 0xb8, 0x6b, 0.13),
            capacity,
        ));
    }

    right = right.push(section_label("NETWORK"));
    for network in &snapshot.networks {
        let net_history = history
            .networks
            .get(&network.name)
            .cloned()
            .unwrap_or_default();
        right = right.push(network_card(
            &network.name,
            rate(network.down_bytes_per_sec),
            rate(network.up_bytes_per_sec),
            &net_history.down,
            &net_history.up,
            capacity,
        ));
    }

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

pub fn panel_height(state: &MonitorState) -> u32 {
    let temperatures = state.snapshot.temperatures.len() as u32;
    let disks = state.snapshot.disks.len() as u32;
    let networks = state.snapshot.networks.len() as u32;

    let left = metric_card_height(state.snapshot.top_memory.len() as u32)
        + SECTION_HEIGHT
        + temperatures * small_graph_card_height()
        + (temperatures + 1) * COLUMN_SPACING;

    let right = metric_card_height(state.snapshot.top_cpu.len() as u32)
        + SECTION_HEIGHT * 2
        + disks * small_graph_card_height()
        + networks * network_card_height()
        + (disks + networks + 2) * COLUMN_SPACING;

    left.max(right).saturating_add(PANEL_PADDING * 2).max(220)
}

fn metric_card_height(process_count: u32) -> u32 {
    let process_body =
        process_count * PROCESS_ROW_HEIGHT + process_count.saturating_sub(1) * PROCESS_SPACING;

    CARD_PADDING * 2
        + METRIC_HEADER_HEIGHT
        + SECONDARY_ROW_HEIGHT
        + PROGRESS_HEIGHT
        + METRIC_GRAPH_HEIGHT
        + process_body
        + METRIC_SPACING * 4
}

const fn small_graph_card_height() -> u32 {
    CARD_PADDING * 2 + SMALL_HEADER_HEIGHT + SMALL_GRAPH_HEIGHT + SMALL_SPACING
}

const fn network_card_height() -> u32 {
    CARD_PADDING * 2
        + SMALL_HEADER_HEIGHT
        + GRAPH_VALUE_ROW_HEIGHT * 2
        + NETWORK_GRAPH_HEIGHT * 2
        + SMALL_SPACING * 4
}

#[allow(clippy::too_many_arguments)]
fn metric_card<'a>(
    title: &'a str,
    value: String,
    subtitle: String,
    secondary_label: &'a str,
    secondary_value: String,
    progress: Option<f32>,
    graph_values: &VecDeque<f64>,
    graph_min: f64,
    graph_max: f64,
    auto_scale: bool,
    graph_color: Color,
    fill_color: Color,
    processes: Vec<(String, String)>,
    capacity: usize,
) -> Element<'a, Message> {
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
    for (name, value) in processes {
        process_body = process_body.push(process_row(name, value));
    }
    content = content.push(process_body);

    card(content)
}

#[allow(clippy::too_many_arguments)]
fn small_graph_card<'a>(
    name: &'a str,
    value: String,
    values: &VecDeque<f64>,
    min: f64,
    max: f64,
    auto_scale: bool,
    graph_color: Color,
    fill_color: Color,
    capacity: usize,
) -> Element<'a, Message> {
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

fn network_card<'a>(
    name: &'a str,
    down_value: String,
    up_value: String,
    down_values: &VecDeque<f64>,
    up_values: &VecDeque<f64>,
    capacity: usize,
) -> Element<'a, Message> {
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

fn process_row(name: String, value: String) -> Element<'static, Message> {
    row![
        container(
            label_owned(name, 12, FG)
                .wrapping(Wrapping::None)
                .width(Fill)
        )
        .width(Fill)
        .clip(true),
        label_owned(value, 12, MUTED)
    ]
    .spacing(5)
    .height(PROCESS_ROW_HEIGHT)
    .align_y(Alignment::Center)
    .into()
}

fn graph_value_row<'a>(label_text: &'a str, value: String, color: Color) -> Element<'a, Message> {
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

fn section_label<'a>(value: &'a str) -> Element<'a, Message> {
    container(bold_label(value, 12, MUTED))
        .height(SECTION_HEIGHT)
        .width(Fill)
        .into()
}

fn card<'a>(content: Column<'a, Message>) -> Element<'a, Message> {
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

fn unavailable() -> String {
    "—".to_owned()
}

fn rate(value: f64) -> String {
    format!("{}/s", bytes(value.max(0.0) as u64))
}

fn bytes(value: u64) -> String {
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

pub fn theme() -> Theme {
    Theme::Dark
}
