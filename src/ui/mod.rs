mod graph;

use std::collections::VecDeque;

use graph::Graph;
use iced::border;
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
const MONO: Font = Font::MONOSPACE;

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
    .spacing(8)
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
    .spacing(8)
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
        .spacing(8)
        .width(Fill)
        .align_y(Alignment::Start);

    container(body)
        .padding(10)
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
    let memory_card = 110 + state.snapshot.top_memory.len() as u32 * 15;
    let cpu_card = 110 + state.snapshot.top_cpu.len() as u32 * 15;
    let left = memory_card + 28 + state.snapshot.temperatures.len() as u32 * 78;
    let right = cpu_card
        + 28
        + state.snapshot.disks.len() as u32 * 78
        + 28
        + state.snapshot.networks.len() as u32 * 126;
    left.max(right).saturating_add(20).max(220)
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
    let header = row![
        label(title, 12, MUTED).width(Fill),
        label_owned(value, 16, FG),
    ]
    .spacing(4)
    .align_y(Alignment::Center);

    let secondary = row![
        label(secondary_label, 11, MUTED).width(Fill),
        label_owned(secondary_value, 11, MUTED),
    ]
    .spacing(4)
    .align_y(Alignment::Center);

    let mut content = column![header].spacing(5);
    if !subtitle.is_empty() {
        content = content.push(
            row![Space::new().width(Fill), label_owned(subtitle, 11, MUTED)]
                .align_y(Alignment::Center),
        );
    }
    content = content.push(secondary);
    if let Some(progress) = progress {
        content = content.push(
            progress_bar(0.0..=1.0, progress.clamp(0.0, 1.0))
                .girth(6)
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
    content = content.push(canvas(graph).width(Fill).height(28));

    for (name, value) in processes {
        content = content.push(process_row(name, value));
    }

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
    let header = row![label(name, 13, FG).width(Fill), label_owned(value, 12, FG)]
        .align_y(Alignment::Center);
    let graph = Graph::new(values, capacity, graph_color, fill_color)
        .range(min, max)
        .auto_scale(auto_scale);
    card(column![header, canvas(graph).width(Fill).height(24)].spacing(4))
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
            label(name, 13, FG),
            graph_value_row("↓", down_value, GREEN),
            canvas(down_graph).width(Fill).height(22),
            graph_value_row("↑", up_value, ORANGE),
            canvas(up_graph).width(Fill).height(22),
        ]
        .spacing(4),
    )
}

fn process_row(name: String, value: String) -> Element<'static, Message> {
    row![
        label_owned(name, 11, FG).width(Fill),
        label_owned(value, 11, MUTED)
    ]
    .spacing(5)
    .align_y(Alignment::Center)
    .into()
}

fn graph_value_row<'a>(label_text: &'a str, value: String, color: Color) -> Element<'a, Message> {
    row![
        label(label_text, 13, color),
        Space::new().width(Fill),
        label_owned(value, 11, color),
    ]
    .spacing(4)
    .align_y(Alignment::Center)
    .into()
}

fn section_label<'a>(value: &'a str) -> Element<'a, Message> {
    container(label(value, 12, MUTED))
        .height(14)
        .width(Fill)
        .into()
}

fn card<'a>(content: Column<'a, Message>) -> Element<'a, Message> {
    container(content)
        .padding(9)
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
