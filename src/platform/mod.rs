use std::str::FromStr;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopCommand {
    Quit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProbeKind {
    Cpu,
    Memory,
    Processes,
    Network,
    Disk,
    Temperatures,
}

impl ProbeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Memory => "memory",
            Self::Processes => "processes",
            Self::Network => "network",
            Self::Disk => "disk",
            Self::Temperatures => "temperatures",
        }
    }
}

impl FromStr for ProbeKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "cpu" => Ok(Self::Cpu),
            "memory" => Ok(Self::Memory),
            "process" | "processes" => Ok(Self::Processes),
            "network" | "networks" => Ok(Self::Network),
            "disk" | "disks" => Ok(Self::Disk),
            "temperature" | "temperatures" => Ok(Self::Temperatures),
            _ => Err(format!(
                "unknown collector {value:?}; expected cpu, memory, processes, network, disk, or temperatures"
            )),
        }
    }
}

pub struct ProbeReport {
    pub available: bool,
    pub summary: Vec<String>,
    pub raw: Vec<String>,
    pub notes: Vec<String>,
}

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{Backend, desktop};

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::Backend;

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
compile_error!("cclover-mon currently supports Linux and Windows targets only");
