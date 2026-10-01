mod io;
mod sensors;
mod system;

pub use io::{DiskPanel, IoProcessRow, NetworkPanel};
pub use sensors::{FanPanel, GpuPanel, TemperaturePanel};
pub use system::{CpuPanel, MemoryPanel, ProcessRow};

#[cfg(test)]
pub(crate) use io::disk_title;
#[cfg(test)]
pub(crate) use sensors::short_temperature_name;
