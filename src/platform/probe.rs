#[cfg(target_os = "windows")]
use crate::core::model::CollectionUnavailable;
#[cfg(any(target_os = "linux", target_os = "windows"))]
use crate::core::model::FanSnapshot;
#[cfg(any(target_os = "linux", target_os = "windows"))]
use crate::core::model::GpuSnapshot;
#[cfg(any(target_os = "linux", target_os = "windows"))]
use crate::core::model::TemperatureSnapshot;
use crate::core::model::{
    Collection, CollectionStatus, CpuCounter, DiskCounter, MemorySnapshot, NetworkCounter,
    ProcessCounter,
};
#[cfg(target_os = "linux")]
use crate::core::model::{ProcessDiskIoCounter, ProcessNetworkIoCounter};
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
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    Temperatures(Collection<Vec<TemperatureSnapshot>>),
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    Fans(Collection<Vec<FanSnapshot>>),
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    Gpu(Collection<Vec<GpuSnapshot>>),
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
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            Self::Temperatures(value) => value.is_observable(),
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            Self::Fans(value) => value.is_observable(),
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            Self::Gpu(value) => value.is_observable(),
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
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            Self::Temperatures(value) => report_temperatures(value, notes),
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            Self::Fans(value) => report_fans(value, notes),
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            Self::Gpu(value) => report_gpus(value, notes),
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

fn unavailable_report(
    status: CollectionStatus,
    summary: impl Into<String>,
    notes: Vec<String>,
) -> ProbeReport {
    ProbeReport {
        status,
        summary: vec![summary.into()],
        raw: Vec::new(),
        notes,
    }
}

fn report_memory(value: Collection<MemorySnapshot>, notes: Vec<String>) -> ProbeReport {
    let status = value.status();
    let Some(memory) = value.value() else {
        return unavailable_report(status, "memory counters unavailable", notes);
    };
    let raw = vec![format!(
        "used_bytes={} total_bytes={} swap_used_bytes={} swap_total_bytes={}",
        memory.used_bytes, memory.total_bytes, memory.swap_used_bytes, memory.swap_total_bytes
    )];
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
    let Some(values) = value.value().map(Vec::as_slice) else {
        return unavailable_report(status, "process counters unavailable", notes);
    };
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
    let Some(values) = value.value().map(Vec::as_slice) else {
        return unavailable_report(status, "network counters unavailable", notes);
    };
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
    let Some(values) = value.value().map(Vec::as_slice) else {
        return unavailable_report(status, "disk counters unavailable", notes);
    };
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

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn report_temperatures(
    value: Collection<Vec<TemperatureSnapshot>>,
    notes: Vec<String>,
) -> ProbeReport {
    let status = value.status();
    let Some(values) = value.value().map(Vec::as_slice) else {
        return unavailable_report(status, "temperature collection unavailable", notes);
    };
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

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn report_fans(value: Collection<Vec<FanSnapshot>>, notes: Vec<String>) -> ProbeReport {
    let status = value.status();
    let Some(values) = value.value().map(Vec::as_slice) else {
        return unavailable_report(status, "fan collection unavailable", notes);
    };
    let raw = values
        .iter()
        .map(|fan| {
            format!(
                "id={} name={} rpm={}",
                fan.id.as_opaque_key(),
                fan.name,
                fan.rpm
            )
        })
        .collect();
    let summary = if values.is_empty() {
        vec!["0 fan sensors".to_owned()]
    } else {
        values
            .iter()
            .map(|fan| format!("{} {} RPM", fan.name, fan.rpm))
            .collect()
    };
    ProbeReport {
        status,
        summary,
        raw,
        notes,
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn report_gpus(value: Collection<Vec<GpuSnapshot>>, notes: Vec<String>) -> ProbeReport {
    let status = value.status();
    let Some(values) = value.value().map(Vec::as_slice) else {
        return unavailable_report(status, "GPU collection unavailable", notes);
    };
    ProbeReport {
        status,
        summary: vec![format!("{} GPUs", values.len())],
        raw: values
            .iter()
            .map(|gpu| {
                format!(
                    "id={} name={} utilization_percent={:?} memory_used_bytes={:?} memory_total_bytes={:?} temperature_celsius={:?} power_watts={:?} core_clock_mhz={:?} fan_percent={:?} fan_rpm={:?}",
                    gpu.id.as_opaque_key(),
                    gpu.name,
                    gpu.utilization_percent,
                    gpu.memory_used_bytes,
                    gpu.memory_total_bytes,
                    gpu.temperature_celsius,
                    gpu.power_watts,
                    gpu.core_clock_mhz,
                    gpu.fan_percent,
                    gpu.fan_rpm,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::model::CollectionUnavailable;

    #[test]
    fn empty_temperature_observation_remains_successful_and_empty() {
        let report = report_temperatures(Collection::available(Vec::new()), Vec::new());

        assert_eq!(report.status, CollectionStatus::Available);
        assert_eq!(report.summary, ["0 temperature sensors"]);
    }

    #[test]
    fn unavailable_temperature_observation_is_not_reported_as_empty() {
        let report = report_temperatures(
            Collection::unavailable(CollectionUnavailable::PermissionDenied),
            Vec::new(),
        );

        assert_eq!(
            report.status,
            CollectionStatus::Unavailable(CollectionUnavailable::PermissionDenied)
        );
        assert_eq!(report.summary, ["temperature collection unavailable"]);
        assert!(report.raw.is_empty());
    }

    #[test]
    fn unavailable_vector_collector_is_not_reported_as_zero_items() {
        let report = report_processes(
            Collection::unavailable(CollectionUnavailable::Unsupported),
            Vec::new(),
        );

        assert_eq!(
            report.status,
            CollectionStatus::Unavailable(CollectionUnavailable::Unsupported)
        );
        assert_eq!(report.summary, ["process counters unavailable"]);
    }
}
