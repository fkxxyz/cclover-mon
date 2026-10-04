use crate::cell_width::{display_width, fit, pad_left, pad_right, pad_right_styled};
use crate::frame::{Frame, IoDevice, Metric, NetworkDevice, Overview};

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const CYAN: &str = "\x1b[36m";

pub(crate) struct RenderedFrame {
    pub(crate) lines: Vec<String>,
}

pub(crate) fn render(frame: &Frame, width: usize, height: usize) -> RenderedFrame {
    if width == 0 || height == 0 {
        return RenderedFrame { lines: Vec::new() };
    }

    let mut lines = Vec::new();
    lines.push(header(width));
    if height == 1 {
        return RenderedFrame { lines };
    }

    let top_n = if width < 100 {
        if height < 32 { 0 } else { 3 }
    } else {
        match height {
            0..=21 => 0,
            22..=27 => 2,
            _ => 3,
        }
    };

    if width >= 100 {
        lines.extend(wide_overview(frame, width, top_n));
        push_blank(&mut lines);
        lines.extend(gpu_lines(frame, width));
        lines.extend(thermal_lines(frame, width));
        push_blank(&mut lines);
        lines.extend(wide_io(frame, width, top_n));
    } else {
        lines.extend(narrow_overview(frame, width, top_n));
        lines.extend(gpu_lines(frame, width));
        lines.extend(thermal_lines(frame, width));
        lines.extend(narrow_io(frame, width, top_n));
    }

    lines.truncate(height);
    RenderedFrame { lines }
}

fn header(width: usize) -> String {
    let left = format!("{BOLD}cclover-mon{RESET}");
    let right = format!("{DIM}q quit{RESET}");
    let left_width = display_width("cclover-mon");
    let right_width = display_width("q quit");
    if width <= left_width + right_width + 1 {
        return fit("cclover-mon", width);
    }
    format!(
        "{left}{}{right}",
        " ".repeat(width - left_width - right_width)
    )
}

fn wide_overview(frame: &Frame, width: usize, top_n: usize) -> Vec<String> {
    let (left, right, gap) = split_width(width);
    let cpu = overview_lines(&frame.cpu, left, top_n, "TOP CPU", true);
    let memory = overview_lines(&frame.memory, right, top_n, "TOP MEMORY", true);
    join_columns(&cpu, &memory, left, right, gap)
}

fn narrow_overview(frame: &Frame, width: usize, top_n: usize) -> Vec<String> {
    let mut lines = overview_lines(&frame.cpu, width, top_n, "TOP CPU", false);
    push_blank(&mut lines);
    lines.extend(overview_lines(
        &frame.memory,
        width,
        top_n,
        "TOP MEMORY",
        false,
    ));
    push_blank(&mut lines);
    lines
}

fn overview_lines(
    panel: &Overview,
    width: usize,
    top_n: usize,
    top_title: &str,
    reserve_secondary: bool,
) -> Vec<String> {
    let mut lines = vec![section_value(panel.title, &panel.value, width)];
    lines.push(style_dim(&sparkline(
        &panel.history,
        panel.history_max,
        width,
    )));
    if let Some(secondary) = &panel.secondary {
        lines.push(metric_line(secondary, width));
    } else if reserve_secondary {
        lines.push(String::new());
    }

    if top_n > 0 {
        lines.push(section_title(top_title, width));
        for process in panel.processes.iter().take(top_n) {
            lines.push(metric_line(process, width));
        }
    }
    lines
}

fn gpu_lines(frame: &Frame, width: usize) -> Vec<String> {
    if frame.gpus.is_empty() {
        return Vec::new();
    }
    let mut lines = vec![section_title("GPU", width)];
    for gpu in &frame.gpus {
        if width >= 100 {
            let plain = format!(
                "{}  Util {}  VRAM {}  Temp {}  Power {}  Core {}",
                gpu.name,
                pad_left(&gpu.utilization, 6),
                pad_left(&gpu.memory, 17),
                pad_left(&gpu.temperature, 7),
                pad_left(&gpu.power, 7),
                pad_left(&gpu.clock, 9)
            );
            lines.push(fit(&plain, width));
        } else {
            lines.push(style_bold(&fit(&gpu.name, width)));
            let plain = format!(
                "Util {}  VRAM {}  Temp {}  Power {}  Core {}",
                gpu.utilization, gpu.memory, gpu.temperature, gpu.power, gpu.clock
            );
            lines.push(style_dim(&fit(&plain, width)));
        }
    }
    push_blank(&mut lines);
    lines
}

fn thermal_lines(frame: &Frame, width: usize) -> Vec<String> {
    if frame.thermals.is_empty() {
        return Vec::new();
    }
    let mut lines = vec![section_title("THERMALS", width)];
    let columns = if width >= 100 {
        3
    } else if width >= 60 {
        2
    } else {
        1
    };
    let gap = 3;
    let cell_width = (width.saturating_sub(gap * (columns - 1))) / columns;
    for chunk in frame.thermals.chunks(columns) {
        let mut line = String::new();
        for (index, metric) in chunk.iter().enumerate() {
            if index > 0 {
                line.push_str(&" ".repeat(gap));
            }
            let cell = metric_plain(metric, cell_width);
            line.push_str(&pad_right(&cell, cell_width));
        }
        lines.push(line.trim_end().to_owned());
    }
    lines
}

fn wide_io(frame: &Frame, width: usize, top_n: usize) -> Vec<String> {
    let (left, right, gap) = split_width(width);
    let disk = disk_lines(&frame.disks, left, top_n);
    let network = network_lines(&frame.networks, right, top_n);
    join_columns(&disk, &network, left, right, gap)
}

fn narrow_io(frame: &Frame, width: usize, top_n: usize) -> Vec<String> {
    let mut lines = disk_lines(&frame.disks, width, top_n);
    if !lines.is_empty() && !frame.networks.is_empty() {
        push_blank(&mut lines);
    }
    lines.extend(network_lines(&frame.networks, width, top_n));
    lines
}

fn disk_lines(disks: &[IoDevice], width: usize, top_n: usize) -> Vec<String> {
    if disks.is_empty() {
        return Vec::new();
    }
    let mut lines = vec![section_title("DISK I/O", width)];
    lines.push(style_dim(&two_column_header("DEVICE", "RATE", width)));
    for disk in disks {
        lines.push(two_column_line(&disk.name, &disk.rate, width));
    }
    if top_n > 0 {
        let mut processes = disks
            .iter()
            .flat_map(|disk| disk.processes.iter())
            .take(top_n)
            .peekable();
        if processes.peek().is_some() {
            lines.push(style_dim(&fit("Top processes", width)));
            for process in processes {
                let value = format!("R {}  W {}", process.first, process.second);
                lines.push(two_column_line(&process.name, &value, width));
            }
        }
    }
    lines
}

fn network_lines(networks: &[NetworkDevice], width: usize, top_n: usize) -> Vec<String> {
    if networks.is_empty() {
        return Vec::new();
    }
    let mut lines = vec![section_title("NETWORK", width)];
    lines.push(style_dim(&three_column_header(
        "INTERFACE",
        "DOWN",
        "UP",
        width,
    )));
    for network in networks {
        lines.push(three_column_line(
            &network.name,
            &network.down,
            &network.up,
            width,
        ));
    }
    if top_n > 0 {
        let mut processes = networks
            .iter()
            .flat_map(|network| network.processes.iter())
            .take(top_n)
            .peekable();
        if processes.peek().is_some() {
            lines.push(style_dim(&fit("Top processes", width)));
            for process in processes {
                lines.push(three_column_line(
                    &process.name,
                    &process.first,
                    &process.second,
                    width,
                ));
            }
        }
    }
    lines
}

fn section_value(title: &str, value: &str, width: usize) -> String {
    let plain = format!("{title}  {value}");
    if display_width(&plain) > width {
        return style_bold(&fit(&plain, width));
    }
    format!("{BOLD}{CYAN}{title}{RESET}  {BOLD}{value}{RESET}")
}

fn section_title(title: &str, width: usize) -> String {
    format!("{BOLD}{CYAN}{}{RESET}", fit(title, width))
}

fn style_bold(text: &str) -> String {
    format!("{BOLD}{text}{RESET}")
}

fn style_dim(text: &str) -> String {
    format!("{DIM}{text}{RESET}")
}

fn metric_line(metric: &Metric, width: usize) -> String {
    two_column_line(&metric.name, &metric.value, width)
}

fn metric_plain(metric: &Metric, width: usize) -> String {
    two_column_line_plain(&metric.name, &metric.value, width)
}

fn two_column_header(left: &str, right: &str, width: usize) -> String {
    two_column_line_plain(left, right, width)
}

fn two_column_line(left: &str, right: &str, width: usize) -> String {
    two_column_line_plain(left, right, width)
}

fn two_column_line_plain(left: &str, right: &str, width: usize) -> String {
    if width < 8 {
        return fit(left, width);
    }
    let right_width = display_width(right).min(width / 2);
    let left_width = width.saturating_sub(right_width + 2);
    let left = pad_right(&fit(left, left_width), left_width);
    let right = pad_left(&fit(right, right_width), right_width);
    format!("{left}  {right}")
}

fn three_column_header(left: &str, middle: &str, right: &str, width: usize) -> String {
    three_column_line(left, middle, right, width)
}

fn three_column_line(left: &str, middle: &str, right: &str, width: usize) -> String {
    if width < 18 {
        return fit(&format!("{left} {middle} {right}"), width);
    }
    let numeric = (width / 4).clamp(7, 12);
    let name = width.saturating_sub(numeric * 2 + 4);
    let left = pad_right(&fit(left, name), name);
    let middle = pad_left(&fit(middle, numeric), numeric);
    let right = pad_left(&fit(right, numeric), numeric);
    format!("{left}  {middle}  {right}")
}

fn split_width(width: usize) -> (usize, usize, usize) {
    let gap = 4;
    let available = width.saturating_sub(gap);
    let left = available / 2;
    (left, available - left, gap)
}

fn join_columns(
    left: &[String],
    right: &[String],
    left_width: usize,
    right_width: usize,
    gap: usize,
) -> Vec<String> {
    let rows = left.len().max(right.len());
    (0..rows)
        .map(|index| {
            let l = left.get(index).map(String::as_str).unwrap_or("");
            let r = right.get(index).map(String::as_str).unwrap_or("");
            format!(
                "{}{}{}",
                pad_right_styled(l, left_width),
                " ".repeat(gap),
                pad_right_styled(r, right_width)
            )
            .trim_end()
            .to_owned()
        })
        .collect()
}

fn sparkline(values: &[f64], maximum: f64, width: usize) -> String {
    const LEVELS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    if width == 0 || values.is_empty() {
        return String::new();
    }
    let maximum = maximum.max(1.0);
    let mut used = 0;
    let mut glyphs = Vec::new();
    for value in values.iter().rev() {
        let fraction = (value.max(0.0) / maximum).clamp(0.0, 1.0);
        let index = (fraction * (LEVELS.len() - 1) as f64).round() as usize;
        let glyph = LEVELS[index];
        let mut encoded = [0; 4];
        let glyph_width = display_width(glyph.encode_utf8(&mut encoded));
        if used + glyph_width > width {
            break;
        }
        glyphs.push(glyph);
        used += glyph_width;
    }
    glyphs.into_iter().rev().collect()
}

fn push_blank(lines: &mut Vec<String>) {
    if lines.last().is_some_and(|line| !line.is_empty()) {
        lines.push(String::new());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{GpuRow, IoDevice, Metric, NetworkDevice, Overview};

    fn sample_frame() -> Frame {
        Frame {
            cpu: Overview {
                title: "CPU",
                value: "12.0%".into(),
                processes: vec![
                    Metric {
                        name: "cpu-a".into(),
                        value: "4.0%".into(),
                    },
                    Metric {
                        name: "cpu-b".into(),
                        value: "3.0%".into(),
                    },
                    Metric {
                        name: "cpu-c".into(),
                        value: "2.0%".into(),
                    },
                ],
                history: vec![10.0, 20.0, 30.0],
                history_max: 100.0,
                ..Overview::default()
            },
            memory: Overview {
                title: "MEMORY",
                value: "8.0 GiB / 32.0 GiB".into(),
                secondary: Some(Metric {
                    name: "SWAP".into(),
                    value: "0 B / 0 B".into(),
                }),
                processes: vec![
                    Metric {
                        name: "mem-a".into(),
                        value: "512 MiB".into(),
                    },
                    Metric {
                        name: "mem-b".into(),
                        value: "256 MiB".into(),
                    },
                    Metric {
                        name: "mem-c".into(),
                        value: "128 MiB".into(),
                    },
                ],
                history: vec![4.0, 6.0, 8.0],
                history_max: 32.0,
            },
            gpus: vec![GpuRow {
                name: "GPU 0".into(),
                utilization: "1.0%".into(),
                memory: "1.0 GiB / 8.0 GiB".into(),
                temperature: "40.0°C".into(),
                power: "20 W".into(),
                clock: "300 MHz".into(),
            }],
            thermals: vec![
                Metric {
                    name: "CPU".into(),
                    value: "50.0°C".into(),
                },
                Metric {
                    name: "NVMe".into(),
                    value: "45.0°C".into(),
                },
            ],
            disks: vec![IoDevice {
                name: "nvme0n1".into(),
                rate: "1.00 MiB/s".into(),
                processes: Vec::new(),
            }],
            networks: vec![NetworkDevice {
                name: "eth0".into(),
                down: "2.00 MiB/s".into(),
                up: "1.00 MiB/s".into(),
                processes: Vec::new(),
            }],
        }
    }

    fn plain_lines(frame: &RenderedFrame) -> Vec<String> {
        frame.lines.iter().map(|line| strip_ansi(line)).collect()
    }

    fn strip_ansi(text: &str) -> String {
        let mut chars = text.chars().peekable();
        let mut out = String::new();
        while let Some(ch) = chars.next() {
            if ch == '\x1b' && chars.peek() == Some(&'[') {
                chars.next();
                for next in chars.by_ref() {
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            } else {
                out.push(ch);
            }
        }
        out
    }

    #[test]
    fn sparkline_uses_fixed_scale() {
        assert_eq!(sparkline(&[0.0, 50.0, 100.0], 100.0, 3), "▁▅█");
    }

    #[test]
    fn wide_layout_keeps_cpu_and_memory_in_parallel_columns() {
        let rendered = render(&sample_frame(), 120, 40);
        let lines = plain_lines(&rendered);
        assert!(lines[1].contains("CPU  12.0%"));
        assert!(lines[1].contains("MEMORY  8.0 GiB / 32.0 GiB"));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("TOP CPU") && line.contains("TOP MEMORY"))
        );
    }

    #[test]
    fn compact_80x24_layout_preserves_core_sections_before_top_processes() {
        let rendered = render(&sample_frame(), 80, 24);
        let lines = plain_lines(&rendered);
        assert!(rendered.lines.len() <= 24);
        for section in ["CPU", "MEMORY", "GPU", "THERMALS", "DISK I/O", "NETWORK"] {
            assert!(
                lines.iter().any(|line| line.contains(section)),
                "missing {section}"
            );
        }
        assert!(
            !lines
                .iter()
                .any(|line| line.contains("TOP CPU") || line.contains("TOP MEMORY"))
        );
    }

    #[test]
    fn compact_layout_restores_top_processes_when_height_allows() {
        let rendered = render(&sample_frame(), 80, 40);
        let lines = plain_lines(&rendered);
        assert!(lines.iter().any(|line| line.contains("TOP CPU")));
        assert!(lines.iter().any(|line| line.contains("TOP MEMORY")));
        assert!(lines.iter().any(|line| line.contains("NETWORK")));
    }

    #[test]
    fn very_short_wide_layout_drops_top_processes_before_core_io() {
        let rendered = render(&sample_frame(), 120, 21);
        let lines = plain_lines(&rendered);
        assert!(rendered.lines.len() <= 21);
        assert!(
            !lines
                .iter()
                .any(|line| line.contains("TOP CPU") || line.contains("TOP MEMORY"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("DISK I/O") && line.contains("NETWORK"))
        );
    }

    #[test]
    fn unicode_content_never_exceeds_terminal_cell_budget() {
        let mut frame = sample_frame();
        frame.cpu.processes[0].name = "编译器e\u{301}".into();
        frame.gpus[0].name = "图形处理器".into();
        frame.thermals[0].name = "处理器温度".into();
        frame.disks[0].name = "数据盘".into();
        frame.networks[0].name = "网络接口".into();

        for width in 1..=120 {
            let rendered = render(&frame, width, 80);
            for line in &rendered.lines {
                assert!(
                    crate::cell_width::styled_width(line) <= width,
                    "line exceeds {width} cells: {line:?}"
                );
            }
        }
    }
}
