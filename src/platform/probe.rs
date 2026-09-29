use crate::core::model::{
    Collection, CpuCounter, DiskCounter, GpuMemorySnapshot, MemorySnapshot, NetworkCounter,
    ProcessCounter,
};
#[cfg(target_os = "windows")]
use crate::core::model::{CollectionStatus, CollectionUnavailable};
#[cfg(target_os = "linux")]
use crate::core::model::{ProcessDiskIoCounter, ProcessNetworkIoCounter, TemperatureSnapshot};
#[cfg(target_os = "windows")]
use crate::platform::ProbeKind;
use crate::platform::ProbeReport;

#[derive(Debug)]
pub(crate) enum ProbeSample {
    Cpu(Collection<CpuCounter>),
    Memory(Collection<MemorySnapshot>),
    Processes(Collection<Vec<ProcessCounter>>),
    Network(Collection<Vec<NetworkCounter>>),
    #[cfg(target_os = "linux")]
    NetworkAttribution(Collection<Vec<ProcessNetworkIoCounter>>),
    Disk(Collection<Vec<DiskCounter>>),
    #[cfg(target_os = "linux")]
    DiskAttribution(Collection<Vec<ProcessDiskIoCounter>>),
    #[cfg(target_os = "linux")]
    Temperatures(Collection<Vec<TemperatureSnapshot>>),
    #[cfg(target_os = "linux")]
    GpuMemory(Collection<Vec<GpuMemorySnapshot>>),
    #[cfg(target_os = "windows")]
    Unsupported(ProbeKind),
}

impl ProbeSample {
    pub(super) fn is_observable(&self) -> bool {
        match self {
            Self::Cpu(value) => value.is_observable(),
            Self::Memory(value) => value.is_observable(),
            Self::Processes(value) => value.is_observable(),
            Self::Network(value) => value.is_observable(),
            #[cfg(target_os = "linux")]
            Self::NetworkAttribution(value) => value.is_observable(),
            Self::Disk(value) => value.is_observable(),
            #[cfg(target_os = "linux")]
            Self::DiskAttribution(value) => value.is_observable(),
            #[cfg(target_os = "linux")]
            Self::Temperatures(value) => value.is_observable(),
            #[cfg(target_os = "linux")]
            Self::GpuMemory(value) => value.is_observable(),
            #[cfg(target_os = "windows")]
            Self::Unsupported(_) => false,
        }
    }

    pub(super) fn into_report(self, notes: Vec<String>) -> ProbeReport {
        match self {
            Self::Cpu(value) => report_cpu(value, notes),
            Self::Memory(value) => report_memory(value, notes),
            Self::Processes(value) => report_processes(value, notes),
            Self::Network(value) => report_networks(value, notes),
            #[cfg(target_os = "linux")]
            Self::NetworkAttribution(value) => report_network_attribution(value, notes),
            Self::Disk(value) => report_disks(value, notes),
            #[cfg(target_os = "linux")]
            Self::DiskAttribution(value) => report_disk_attribution(value, notes),
            #[cfg(target_os = "linux")]
            Self::Temperatures(value) => report_temperatures(value, notes),
            #[cfg(target_os = "linux")]
            Self::GpuMemory(value) => report_gpu_memory(value, notes),
            #[cfg(target_os = "windows")]
            Self::Unsupported(kind) => ProbeReport {
                status: CollectionStatus::Unavailable(CollectionUnavailable::Unsupported),
                summary: vec![format!(
                    "{} collector is not implemented on Windows yet",
                    kind.as_str()
                )],
                raw: Vec::new(),
                notes,
            },
        }
    }
}

fn report_cpu(value: Collection<CpuCounter>, notes: Vec<String>) -> ProbeReport {
    let status = value.status();
    let summary = value
        .value()
        .map(|cpu| vec![format!("{} logical CPUs", cpu.logical_cpu_count)])
        .unwrap_or_else(|| vec!["CPU counters unavailable".to_owned()]);
    let raw = value
        .value()
        .map(|cpu| {
            vec![format!(
                "total_time_units={} idle_time_units={} logical_cpu_count={}",
                cpu.total_time_units, cpu.idle_time_units, cpu.logical_cpu_count
            )]
        })
        .unwrap_or_default();
    ProbeReport {
        status,
        summary,
        raw,
        notes,
    }
}

fn report_memory(value: Collection<MemorySnapshot>, notes: Vec<String>) -> ProbeReport {
    let status = value.status();
    let raw = value
        .value()
        .map(|memory| {
            vec![format!(
                "used_bytes={} total_bytes={} swap_used_bytes={} swap_total_bytes={}",
                memory.used_bytes,
                memory.total_bytes,
                memory.swap_used_bytes,
                memory.swap_total_bytes
            )]
        })
        .unwrap_or_default();
    #[cfg(target_os = "linux")]
    let summary = if raw.is_empty() {
        vec!["memory counters unavailable".to_owned()]
    } else {
        raw.clone()
    };
    #[cfg(target_os = "windows")]
    let summary = raw.clone();
    ProbeReport {
        status,
        summary,
        raw,
        notes,
    }
}

fn report_processes(value: Collection<Vec<ProcessCounter>>, notes: Vec<String>) -> ProbeReport {
    let status = value.status();
    let values = value.value().map(Vec::as_slice).unwrap_or_default();
    ProbeReport {
        status,
        summary: vec![format!("{} processes", values.len())],
        raw: values
            .iter()
            .map(|process| {
                format!(
                    "pid={} name={:?} cpu_time_units={} rss_bytes={}",
                    process.process.pid, process.name, process.cpu_time_units, process.rss_bytes
                )
            })
            .collect(),
        notes,
    }
}

fn report_networks(value: Collection<Vec<NetworkCounter>>, notes: Vec<String>) -> ProbeReport {
    let status = value.status();
    let values = value.value().map(Vec::as_slice).unwrap_or_default();
    #[cfg(target_os = "linux")]
    let summary = vec![format!(
        "{} interfaces: {}",
        values.len(),
        names(values.iter().map(|item| item.name.as_str()))
    )];
    #[cfg(target_os = "windows")]
    let summary = vec![format!("{} interfaces", values.len())];
    ProbeReport {
        status,
        summary,
        raw: values
            .iter()
            .map(|item| {
                format!(
                    "name={} rx_bytes={} tx_bytes={}",
                    item.name, item.rx_bytes, item.tx_bytes
                )
            })
            .collect(),
        notes,
    }
}

#[cfg(target_os = "linux")]
fn report_network_attribution(
    value: Collection<Vec<ProcessNetworkIoCounter>>,
    notes: Vec<String>,
) -> ProbeReport {
    let status = value.status();
    let Some(values) = value.value() else {
        return ProbeReport {
            status,
            summary: vec!["network attribution unavailable".to_owned()],
            raw: Vec::new(),
            notes,
        };
    };
    ProbeReport {
        status,
        summary: vec![format!("{} PID×interface rows", values.len())],
        raw: values
            .iter()
            .map(|row| {
                format!(
                    "pid={} interface={} rx_bytes={} tx_bytes={}",
                    row.process.pid, row.interface, row.rx_bytes, row.tx_bytes
                )
            })
            .collect(),
        notes,
    }
}

fn report_disks(value: Collection<Vec<DiskCounter>>, notes: Vec<String>) -> ProbeReport {
    let status = value.status();
    let values = value.value().map(Vec::as_slice).unwrap_or_default();
    #[cfg(target_os = "linux")]
    let summary = vec![format!(
        "{} devices: {}",
        values.len(),
        names(values.iter().map(|item| item.name.as_str()))
    )];
    #[cfg(target_os = "windows")]
    let summary = vec![format!("{} disks", values.len())];
    ProbeReport {
        status,
        summary,
        raw: values
            .iter()
            .map(|item| {
                format!(
                    "name={} read_bytes={} write_bytes={}",
                    item.name, item.read_bytes, item.write_bytes
                )
            })
            .collect(),
        notes,
    }
}

#[cfg(target_os = "linux")]
fn report_disk_attribution(
    value: Collection<Vec<ProcessDiskIoCounter>>,
    notes: Vec<String>,
) -> ProbeReport {
    let status = value.status();
    let Some(values) = value.value() else {
        return ProbeReport {
            status,
            summary: vec!["disk attribution unavailable".to_owned()],
            raw: Vec::new(),
            notes,
        };
    };
    ProbeReport {
        status,
        summary: vec![format!("{} PID×device rows", values.len())],
        raw: values
            .iter()
            .map(|row| {
                format!(
                    "pid={} device={} read_bytes={} write_bytes={}",
                    row.process.pid, row.device, row.read_bytes, row.write_bytes
                )
            })
            .collect(),
        notes,
    }
}

#[cfg(target_os = "linux")]
fn report_temperatures(
    value: Collection<Vec<TemperatureSnapshot>>,
    notes: Vec<String>,
) -> ProbeReport {
    let status = value.status();
    let values = value.value().map(Vec::as_slice).unwrap_or_default();
    let raw = values
        .iter()
        .map(|temperature| {
            format!(
                "id={} name={} celsius={:.3}",
                temperature.id, temperature.name, temperature.celsius
            )
        })
        .collect();
    let summary = if values.is_empty() {
        vec!["0 temperature sensors".to_owned()]
    } else {
        values
            .iter()
            .map(|temperature| format!("{} {:.1}°C", temperature.name, temperature.celsius))
            .collect()
    };
    ProbeReport {
        status,
        summary,
        raw,
        notes,
    }
}

#[cfg(target_os = "linux")]
fn report_gpu_memory(value: Collection<Vec<GpuMemorySnapshot>>, notes: Vec<String>) -> ProbeReport {
    let status = value.status();
    let values = value.value().map(Vec::as_slice).unwrap_or_default();
    ProbeReport {
        status,
        summary: vec![format!("{} GPU memory devices", values.len())],
        raw: values
            .iter()
            .map(|gpu| {
                format!(
                    "id={} name={} used_bytes={} total_bytes={}",
                    gpu.id.as_opaque_key(),
                    gpu.name,
                    gpu.used_bytes,
                    gpu.total_bytes
                )
            })
            .collect(),
        notes,
    }
}

#[cfg(target_os = "linux")]
fn names<'a>(values: impl Iterator<Item = &'a str>) -> String {
    let names = values.collect::<Vec<_>>();
    if names.is_empty() {
        "none".to_owned()
    } else {
        names.join(", ")
    }
}
