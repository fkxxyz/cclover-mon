use crate::presentation::{DISK_SECTION, Dashboard, NETWORK_SECTION, TEMPERATURE_SECTION};

pub const PANEL_WIDTH: u32 = 390;
pub const INITIAL_PANEL_HEIGHT: u32 = 480;

pub(super) const PANEL_PADDING: u32 = 10;
pub(super) const COLUMN_SPACING: u32 = 8;
pub(super) const CARD_PADDING: u32 = 9;
pub(super) const METRIC_SPACING: u32 = 5;
pub(super) const PROCESS_SPACING: u32 = 2;
pub(super) const SMALL_SPACING: u32 = 4;
pub(super) const METRIC_HEADER_HEIGHT: u32 = 21;
pub(super) const SECONDARY_ROW_HEIGHT: u32 = 15;
pub(super) const PROCESS_ROW_HEIGHT: u32 = 17;
pub(super) const SMALL_HEADER_HEIGHT: u32 = 17;
pub(super) const GRAPH_VALUE_ROW_HEIGHT: u32 = 17;
pub(super) const SECTION_HEIGHT: u32 = 14;
pub(super) const PROGRESS_HEIGHT: u32 = 6;
pub(super) const METRIC_GRAPH_HEIGHT: u32 = 28;
pub(super) const SMALL_GRAPH_HEIGHT: u32 = 24;
pub(super) const NETWORK_GRAPH_HEIGHT: u32 = 22;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Section {
    Temperature,
    Disk,
    Network,
}

impl Section {
    pub(super) const fn title(self) -> &'static str {
        match self {
            Self::Temperature => TEMPERATURE_SECTION,
            Self::Disk => DISK_SECTION,
            Self::Network => NETWORK_SECTION,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PanelBlock {
    Memory { process_count: usize },
    Cpu { process_count: usize },
    Section(Section),
    Temperature(usize),
    Disk(usize),
    Network(usize),
}

impl PanelBlock {
    fn height(self) -> u32 {
        match self {
            Self::Memory { process_count } | Self::Cpu { process_count } => {
                metric_card_height(process_count as u32)
            }
            Self::Section(_) => SECTION_HEIGHT,
            Self::Temperature(_) | Self::Disk(_) => small_graph_card_height(),
            Self::Network(_) => network_card_height(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelLayout {
    memory_process_count: usize,
    cpu_process_count: usize,
    temperature_count: usize,
    disk_count: usize,
    network_count: usize,
}

impl PanelLayout {
    pub fn new(dashboard: Dashboard<'_>) -> Self {
        Self {
            memory_process_count: dashboard.memory().process_count(),
            cpu_process_count: dashboard.cpu().process_count(),
            temperature_count: dashboard.temperature_count(),
            disk_count: dashboard.disk_count(),
            network_count: dashboard.network_count(),
        }
    }

    pub fn height(&self) -> u32 {
        column_height(self.left_blocks())
            .max(column_height(self.right_blocks()))
            .saturating_add(PANEL_PADDING * 2)
            .max(220)
    }

    pub(super) fn left_blocks(&self) -> impl Iterator<Item = PanelBlock> + '_ {
        std::iter::once(PanelBlock::Memory {
            process_count: self.memory_process_count,
        })
        .chain(std::iter::once(PanelBlock::Section(Section::Temperature)))
        .chain((0..self.temperature_count).map(PanelBlock::Temperature))
    }

    pub(super) fn right_blocks(&self) -> impl Iterator<Item = PanelBlock> + '_ {
        std::iter::once(PanelBlock::Cpu {
            process_count: self.cpu_process_count,
        })
        .chain(std::iter::once(PanelBlock::Section(Section::Disk)))
        .chain((0..self.disk_count).map(PanelBlock::Disk))
        .chain(std::iter::once(PanelBlock::Section(Section::Network)))
        .chain((0..self.network_count).map(PanelBlock::Network))
    }
}

fn column_height(blocks: impl Iterator<Item = PanelBlock>) -> u32 {
    let (content, count) = blocks.fold((0_u32, 0_u32), |(height, count), block| {
        (height.saturating_add(block.height()), count + 1)
    });
    let spacing = count.saturating_sub(1) * COLUMN_SPACING;
    content.saturating_add(spacing)
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

#[cfg(test)]
mod tests {
    use crate::core::model::{MonitorState, TemperatureSnapshot};

    use super::*;

    #[test]
    fn layout_structure_drives_dynamic_height() {
        let empty = MonitorState::default();
        let empty_layout = PanelLayout::new(Dashboard::new(&empty));
        assert_eq!(empty_layout.left_blocks().count(), 2);
        assert_eq!(empty_layout.right_blocks().count(), 3);

        let mut populated = MonitorState::default();
        populated.snapshot.temperatures.push(TemperatureSnapshot {
            name: "CPU".to_owned(),
            celsius: 50.0,
        });
        let populated_layout = PanelLayout::new(Dashboard::new(&populated));

        assert_eq!(populated_layout.left_blocks().count(), 3);
        assert!(populated_layout.height() > empty_layout.height());
    }
}
