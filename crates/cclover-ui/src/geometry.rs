pub const PANEL_WIDTH: u32 = 390;
pub const INITIAL_PANEL_HEIGHT: u32 = 480;

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
pub struct TextSlotGeometry {
    pub metric_value: u32,
    pub metric_subtitle: u32,
    pub secondary_value: u32,
    pub process_value: u32,
    pub card_value: u32,
    pub gpu_value: u32,
    pub network_value: u32,
    pub io_pid: u32,
    pub io_value: u32,
}

pub const TEXT_SLOT_GEOMETRY: TextSlotGeometry = TextSlotGeometry {
    metric_value: 58,
    metric_subtitle: 30,
    secondary_value: 62,
    process_value: 58,
    card_value: 56,
    gpu_value: 64,
    network_value: 70,
    io_pid: 32,
    io_value: 43,
};

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
