use std::io;
use std::time::{Duration, Instant};

use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph, Sparkline};

use cclover_presentation::{CpuPanel, Dashboard, MemoryPanel};

pub struct TerminalUi {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl TerminalUi {
    pub fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        if let Err(error) = execute!(stdout, EnterAlternateScreen, Hide) {
            let _ = disable_raw_mode();
            return Err(error);
        }

        match Terminal::new(CrosstermBackend::new(stdout)) {
            Ok(terminal) => Ok(Self { terminal }),
            Err(error) => {
                let _ = disable_raw_mode();
                let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
                Err(error)
            }
        }
    }

    pub fn draw(&mut self, dashboard: Dashboard<'_>) -> io::Result<()> {
        self.terminal.draw(|frame| render(frame, dashboard))?;
        Ok(())
    }

    pub fn wait_for_quit(&mut self, wait: Duration) -> io::Result<bool> {
        let deadline = Instant::now() + wait;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if !event::poll(remaining)? {
                return Ok(false);
            }

            if let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
                && (matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
                    || (key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)))
            {
                return Ok(true);
            }

            if remaining.is_zero() {
                return Ok(false);
            }
        }
    }
}

impl Drop for TerminalUi {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), Show, LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

fn render(frame: &mut ratatui::Frame<'_>, dashboard: Dashboard<'_>) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(9),
            Constraint::Length(5),
            Constraint::Min(8),
        ])
        .split(frame.area());

    frame.render_widget(
        Paragraph::new("cclover-mon  ·  q / Esc / Ctrl-C: quit"),
        sections[0],
    );

    let overview = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(sections[1]);
    render_cpu(frame, overview[0], dashboard.cpu());
    render_memory(frame, overview[1], dashboard.memory());
    render_temperatures(frame, sections[2], dashboard);

    let io = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(sections[3]);
    render_disks(frame, io[0], dashboard);
    render_networks(frame, io[1], dashboard);
}

fn render_cpu(frame: &mut ratatui::Frame<'_>, area: Rect, panel: CpuPanel<'_>) {
    let inner = bordered_inner(frame, area, format!("CPU  {}", panel.value()));
    if inner.height == 0 {
        return;
    }

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(inner);

    let history = sparkline_values(panel.graph_values());
    frame.render_widget(Sparkline::default().data(&history), rows[0]);

    let lines: Vec<_> = panel
        .processes()
        .take(rows[1].height as usize)
        .map(|process| Line::from(format!("{}  {}", process.name, process.value)))
        .collect();
    frame.render_widget(Paragraph::new(lines), rows[1]);
}

fn render_memory(frame: &mut ratatui::Frame<'_>, area: Rect, panel: MemoryPanel<'_>) {
    let title = format!("MEMORY  {} {}", panel.value(), panel.subtitle());
    let inner = bordered_inner(frame, area, title);
    if inner.height == 0 {
        return;
    }

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(inner);

    let history = sparkline_values(panel.graph_values());
    frame.render_widget(Sparkline::default().data(&history), rows[0]);

    let mut lines = vec![Line::from(format!(
        "{}  {}",
        MemoryPanel::SECONDARY_LABEL,
        panel.secondary_value()
    ))];
    lines.extend(
        panel
            .processes()
            .take(rows[1].height.saturating_sub(1) as usize)
            .map(|process| Line::from(format!("{}  {}", process.name, process.value))),
    );
    frame.render_widget(Paragraph::new(lines), rows[1]);
}

fn render_temperatures(frame: &mut ratatui::Frame<'_>, area: Rect, dashboard: Dashboard<'_>) {
    let inner = bordered_inner(frame, area, "TEMPERATURE".to_owned());
    let lines: Vec<_> = (0..dashboard.temperature_count())
        .filter_map(|index| dashboard.temperature(index))
        .map(|temperature| Line::from(format!("{}  {}", temperature.name(), temperature.value())))
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_disks(frame: &mut ratatui::Frame<'_>, area: Rect, dashboard: Dashboard<'_>) {
    let inner = bordered_inner(frame, area, "DISK I/O".to_owned());
    let mut lines = Vec::new();

    for index in 0..dashboard.disk_count() {
        let Some(disk) = dashboard.disk(index) else {
            continue;
        };
        lines.push(Line::from(format!("{}  {}", disk.name(), disk.value())));
        lines.extend(disk.processes().map(|process| {
            Line::from(format!(
                "  {}  R {}  W {}",
                process.name.unwrap_or("?"),
                process.first_value,
                process.second_value
            ))
        }));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_networks(frame: &mut ratatui::Frame<'_>, area: Rect, dashboard: Dashboard<'_>) {
    let inner = bordered_inner(frame, area, "NETWORK".to_owned());
    let mut lines = Vec::new();

    for index in 0..dashboard.network_count() {
        let Some(network) = dashboard.network(index) else {
            continue;
        };
        lines.push(Line::from(format!(
            "{}  ↓ {}  ↑ {}",
            network.name(),
            network.down_value(),
            network.up_value()
        )));
        lines.extend(network.processes().map(|process| {
            Line::from(format!(
                "  {}  ↓ {}  ↑ {}",
                process.name.unwrap_or("?"),
                process.first_value,
                process.second_value
            ))
        }));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn bordered_inner(frame: &mut ratatui::Frame<'_>, area: Rect, title: String) -> Rect {
    let block = Block::default().borders(Borders::ALL).title(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    inner
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
