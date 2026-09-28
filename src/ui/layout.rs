use crate::presentation::{DISK_SECTION, Dashboard, NETWORK_SECTION, TEMPERATURE_SECTION};

pub const PANEL_WIDTH: u32 = 390;
pub const INITIAL_PANEL_HEIGHT: u32 = 480;

#[derive(Debug, Clone, Copy)]
pub(super) struct PanelGeometry {
    pub padding: u32,
    pub column_spacing: u32,
    pub section_height: u32,
    pub min_height: u32,
}

pub(super) const PANEL_GEOMETRY: PanelGeometry = PanelGeometry {
    padding: 10,
    column_spacing: 8,
    section_height: 14,
    min_height: 220,
};

#[derive(Debug, Clone, Copy)]
pub(super) struct CardFrameGeometry {
    pub padding: u32,
}

impl CardFrameGeometry {
    const fn height_with_content(self, content_height: u32) -> u32 {
        self.padding * 2 + content_height
    }
}

pub(super) const CARD_FRAME_GEOMETRY: CardFrameGeometry = CardFrameGeometry { padding: 9 };

#[derive(Debug, Clone, Copy)]
pub(super) struct MetricCardGeometry {
    pub spacing: u32,
    pub process_spacing: u32,
    pub header_height: u32,
    pub secondary_row_height: u32,
    pub process_row_height: u32,
    pub progress_height: u32,
    pub graph_height: u32,
}

impl MetricCardGeometry {
    pub(super) const fn height(self, process_count: u32) -> u32 {
        let process_body = process_count * self.process_row_height
            + process_count.saturating_sub(1) * self.process_spacing;
        let content_height = self.header_height
            + self.secondary_row_height
            + self.progress_height
            + self.graph_height
            + process_body
            + self.spacing * 4;
        CARD_FRAME_GEOMETRY.height_with_content(content_height)
    }
}

pub(super) const METRIC_CARD_GEOMETRY: MetricCardGeometry = MetricCardGeometry {
    spacing: 5,
    process_spacing: 2,
    header_height: 21,
    secondary_row_height: 15,
    process_row_height: 17,
    progress_height: 6,
    graph_height: 28,
};

#[derive(Debug, Clone, Copy)]
pub(super) struct SmallGraphCardGeometry {
    pub spacing: u32,
    pub header_height: u32,
    pub graph_height: u32,
}

impl SmallGraphCardGeometry {
    pub(super) const fn height(self) -> u32 {
        CARD_FRAME_GEOMETRY
            .height_with_content(self.header_height + self.graph_height + self.spacing)
    }
}

pub(super) const SMALL_GRAPH_CARD_GEOMETRY: SmallGraphCardGeometry = SmallGraphCardGeometry {
    spacing: 4,
    header_height: 17,
    graph_height: 24,
};

#[derive(Debug, Clone, Copy)]
pub(super) struct IoProcessGeometry {
    pub spacing: u32,
    pub row_height: u32,
    pub slots: u32,
}

impl IoProcessGeometry {
    pub(super) const fn height(self) -> u32 {
        self.slots * self.row_height + self.slots.saturating_sub(1) * self.spacing
    }
}

pub(super) const IO_PROCESS_GEOMETRY: IoProcessGeometry = IoProcessGeometry {
    spacing: 2,
    row_height: 17,
    slots: 3,
};

#[derive(Debug, Clone, Copy)]
pub(super) struct DiskCardGeometry {
    pub spacing: u32,
    pub header_height: u32,
    pub graph_height: u32,
}

impl DiskCardGeometry {
    pub(super) const fn height(self) -> u32 {
        let content_height = self.header_height
            + self.graph_height
            + IO_PROCESS_GEOMETRY.height()
            + self.spacing * 2;
        CARD_FRAME_GEOMETRY.height_with_content(content_height)
    }
}

pub(super) const DISK_CARD_GEOMETRY: DiskCardGeometry = DiskCardGeometry {
    spacing: 4,
    header_height: 17,
    graph_height: 24,
};

#[derive(Debug, Clone, Copy)]
pub(super) struct NetworkCardGeometry {
    pub spacing: u32,
    pub header_height: u32,
    pub value_row_height: u32,
    pub graph_height: u32,
}

impl NetworkCardGeometry {
    pub(super) const fn height(self) -> u32 {
        let content_height = self.header_height
            + self.value_row_height * 2
            + self.graph_height * 2
            + IO_PROCESS_GEOMETRY.height()
            + self.spacing * 5;
        CARD_FRAME_GEOMETRY.height_with_content(content_height)
    }
}

pub(super) const NETWORK_CARD_GEOMETRY: NetworkCardGeometry = NetworkCardGeometry {
    spacing: 4,
    header_height: 17,
    value_row_height: 17,
    graph_height: 22,
};

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
                METRIC_CARD_GEOMETRY.height(process_count as u32)
            }
            Self::Section(_) => PANEL_GEOMETRY.section_height,
            Self::Temperature(_) => SMALL_GRAPH_CARD_GEOMETRY.height(),
            Self::Disk(_) => DISK_CARD_GEOMETRY.height(),
            Self::Network(_) => NETWORK_CARD_GEOMETRY.height(),
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
            .saturating_add(PANEL_GEOMETRY.padding * 2)
            .max(PANEL_GEOMETRY.min_height)
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
    let spacing = count.saturating_sub(1) * PANEL_GEOMETRY.column_spacing;
    content.saturating_add(spacing)
}

#[cfg(test)]
mod tests {
    use crate::core::model::{Collection, MonitorState, TemperatureSnapshot};

    use super::*;

    #[test]
    fn layout_structure_drives_dynamic_height() {
        let empty = MonitorState::default();
        let empty_layout = PanelLayout::new(Dashboard::new(&empty));
        assert_eq!(empty_layout.left_blocks().count(), 2);
        assert_eq!(empty_layout.right_blocks().count(), 3);

        let mut populated = MonitorState::default();
        populated.snapshot.temperatures = Collection::available(vec![TemperatureSnapshot {
            id: "cpu-temperature".to_owned(),
            name: "CPU".to_owned(),
            celsius: 50.0,
        }]);
        let populated_layout = PanelLayout::new(Dashboard::new(&populated));

        assert_eq!(populated_layout.left_blocks().count(), 3);
        assert!(populated_layout.height() > empty_layout.height());
    }

    #[test]
    fn card_geometry_derives_height_from_render_dimensions() {
        assert_eq!(
            METRIC_CARD_GEOMETRY.height(1) - METRIC_CARD_GEOMETRY.height(0),
            METRIC_CARD_GEOMETRY.process_row_height
        );
        assert_eq!(
            METRIC_CARD_GEOMETRY.height(2) - METRIC_CARD_GEOMETRY.height(1),
            METRIC_CARD_GEOMETRY.process_row_height + METRIC_CARD_GEOMETRY.process_spacing
        );

        let small_content = SMALL_GRAPH_CARD_GEOMETRY.header_height
            + SMALL_GRAPH_CARD_GEOMETRY.graph_height
            + SMALL_GRAPH_CARD_GEOMETRY.spacing;
        assert_eq!(
            SMALL_GRAPH_CARD_GEOMETRY.height(),
            CARD_FRAME_GEOMETRY.height_with_content(small_content)
        );

        let network_content = NETWORK_CARD_GEOMETRY.header_height
            + NETWORK_CARD_GEOMETRY.value_row_height * 2
            + NETWORK_CARD_GEOMETRY.graph_height * 2
            + IO_PROCESS_GEOMETRY.height()
            + NETWORK_CARD_GEOMETRY.spacing * 5;
        assert_eq!(
            NETWORK_CARD_GEOMETRY.height(),
            CARD_FRAME_GEOMETRY.height_with_content(network_content)
        );

        let disk_content = DISK_CARD_GEOMETRY.header_height
            + DISK_CARD_GEOMETRY.graph_height
            + IO_PROCESS_GEOMETRY.height()
            + DISK_CARD_GEOMETRY.spacing * 2;
        assert_eq!(
            DISK_CARD_GEOMETRY.height(),
            CARD_FRAME_GEOMETRY.height_with_content(disk_content)
        );
    }
}
