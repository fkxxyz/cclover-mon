use std::collections::VecDeque;

use cclover_core::model::{
    Collection, DiskSnapshot, FanSnapshot, GpuSnapshot, MemorySnapshot, MonitorState,
    NetworkDirectionHistory, NetworkSnapshot, ProcessCpuUsage, ProcessDiskIo, ProcessMemoryUsage,
    ProcessNetworkIo, TemperatureSnapshot,
};

mod dashboard;
mod format;
mod panels;

pub use dashboard::Dashboard;
pub(crate) use format::format_compact_rate;
pub use format::{BoundedText, format_bytes, format_percent, format_rate, unavailable};
pub use panels::{
    CpuPanel, DiskPanel, FanPanel, GpuPanel, IoProcessRow, MemoryPanel, NetworkPanel, ProcessRow,
    TemperaturePanel,
};
#[cfg(test)]
pub(crate) use panels::{disk_title, short_temperature_name};

pub const TEMPERATURE_SECTION: &str = "TEMPERATURE";
pub const FAN_SECTION: &str = "FAN";
pub const GPU_SECTION: &str = "GPU";
pub const DISK_SECTION: &str = "DISK I/O";
pub const NETWORK_SECTION: &str = "NETWORK";

#[cfg(test)]
mod tests;
