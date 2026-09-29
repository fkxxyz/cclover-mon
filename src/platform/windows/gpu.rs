use crate::core::model::{Collection, CollectionUnavailable, GpuId, GpuSnapshot};

use super::diagnostics::report_issue;
use super::{gpu_adl, gpu_nvml};

pub(super) struct Collector {
    nvml: Source<gpu_nvml::Session>,
    adl: Source<gpu_adl::Session>,
}

enum Source<T> {
    Uninitialized,
    Available { session: T, degraded: bool },
    Unsupported(String),
}

impl Collector {
    pub(super) fn new() -> Self {
        Self {
            nvml: Source::Uninitialized,
            adl: Source::Uninitialized,
        }
    }

    pub(super) fn collect(
        &mut self,
        mut notes: Option<&mut Vec<String>>,
    ) -> Collection<Vec<GpuSnapshot>> {
        initialize_source(&mut self.nvml, gpu_nvml::Session::load, "NVML", &mut notes);
        initialize_source(&mut self.adl, gpu_adl::Session::load, "ADL", &mut notes);

        let nvml_available = matches!(self.nvml, Source::Available { .. });
        let adl_available = matches!(self.adl, Source::Available { .. });
        if !nvml_available && !adl_available {
            report_unsupported(&self.nvml, "NVML", &mut notes);
            report_unsupported(&self.adl, "ADL", &mut notes);
            return Collection::unavailable(CollectionUnavailable::Unsupported);
        }

        let mut degraded = !(nvml_available && adl_available);
        let mut values = Vec::new();
        if let Source::Available {
            session,
            degraded: source_degraded,
        } = &self.nvml
        {
            degraded |= source_degraded;
            collect_nvml(session, &mut values, &mut degraded, &mut notes);
        } else {
            report_unsupported(&self.nvml, "NVML", &mut notes);
        }
        if let Source::Available {
            session,
            degraded: source_degraded,
        } = &self.adl
        {
            degraded |= source_degraded;
            collect_adl(session, &mut values, &mut degraded, &mut notes);
        } else {
            report_unsupported(&self.adl, "ADL", &mut notes);
        }

        if degraded {
            Collection::degraded(values)
        } else {
            Collection::available(values)
        }
    }
}

impl Default for Collector {
    fn default() -> Self {
        Self::new()
    }
}

fn initialize_source<T>(
    source: &mut Source<T>,
    load: impl FnOnce() -> Result<(T, Vec<String>), String>,
    label: &str,
    notes: &mut Option<&mut Vec<String>>,
) {
    if !matches!(source, Source::Uninitialized) {
        return;
    }
    *source = match load() {
        Ok((session, issues)) => {
            let degraded = !issues.is_empty();
            for issue in issues {
                report_issue(notes, || format!("Windows {label}: {issue}"));
            }
            Source::Available { session, degraded }
        }
        Err(reason) => Source::Unsupported(reason),
    };
}

fn report_unsupported<T>(source: &Source<T>, label: &str, notes: &mut Option<&mut Vec<String>>) {
    if let Source::Unsupported(reason) = source {
        report_issue(notes, || format!("Windows {label} unavailable: {reason}"));
    }
}

fn collect_nvml(
    session: &gpu_nvml::Session,
    values: &mut Vec<GpuSnapshot>,
    degraded: &mut bool,
    notes: &mut Option<&mut Vec<String>>,
) {
    values.reserve(session.devices().len());
    for (index, device) in session.devices().iter().enumerate() {
        let utilization_percent = nvml_query(
            degraded,
            notes,
            device.uuid(),
            "utilization",
            session.utilization(index).map(|value| f64::from(value.gpu)),
        );
        let memory = nvml_query(
            degraded,
            notes,
            device.uuid(),
            "memory",
            session.memory(index),
        );
        let temperature_celsius = nvml_query(
            degraded,
            notes,
            device.uuid(),
            "temperature",
            session.temperature(index).map(f64::from),
        );
        let power_watts = nvml_query(
            degraded,
            notes,
            device.uuid(),
            "power",
            session
                .power_milliwatts(index)
                .map(|value| f64::from(value) / 1000.0),
        );
        let core_clock_mhz = nvml_query(
            degraded,
            notes,
            device.uuid(),
            "graphics clock",
            session.graphics_clock_mhz(index).map(u64::from),
        );
        let fan_percent = nvml_query(
            degraded,
            notes,
            device.uuid(),
            "fan speed",
            session.fan_percent(index).map(f64::from),
        );

        values.push(GpuSnapshot {
            id: GpuId::from_opaque_key(format!("nvml:{}", device.uuid())),
            name: device.name().to_owned(),
            utilization_percent,
            memory_used_bytes: memory.as_ref().map(|value| value.used),
            memory_total_bytes: memory.as_ref().map(|value| value.total),
            temperature_celsius,
            power_watts,
            core_clock_mhz,
            fan_percent,
            fan_rpm: None,
        });
    }
}

fn collect_adl(
    session: &gpu_adl::Session,
    values: &mut Vec<GpuSnapshot>,
    degraded: &mut bool,
    notes: &mut Option<&mut Vec<String>>,
) {
    values.reserve(session.devices().len());
    for (index, device) in session.devices().iter().enumerate() {
        let memory_used_bytes = match session.memory_used_bytes(index) {
            Ok(value) => value,
            Err(status) => {
                *degraded = true;
                report_issue(notes, || {
                    format!(
                        "ADL GPU {} dedicated VRAM usage unavailable: status {status}",
                        device.stable_key()
                    )
                });
                None
            }
        };
        let telemetry = match session.telemetry(index) {
            Ok(value) => value,
            Err(status) => {
                *degraded = true;
                report_issue(notes, || {
                    format!(
                        "ADL GPU {} telemetry unavailable: status {status}",
                        device.stable_key()
                    )
                });
                None
            }
        };
        if memory_used_bytes.is_none()
            || telemetry.is_none()
            || device.memory_total_bytes().is_none()
        {
            *degraded = true;
        }

        values.push(GpuSnapshot {
            id: GpuId::from_opaque_key(format!("adl:{}", device.stable_key())),
            name: device.name().to_owned(),
            utilization_percent: telemetry
                .as_ref()
                .and_then(|value| value.utilization_percent),
            memory_used_bytes,
            memory_total_bytes: device.memory_total_bytes(),
            temperature_celsius: telemetry
                .as_ref()
                .and_then(|value| value.temperature_celsius),
            power_watts: telemetry.as_ref().and_then(|value| value.power_watts),
            core_clock_mhz: telemetry.as_ref().and_then(|value| value.core_clock_mhz),
            fan_percent: telemetry.as_ref().and_then(|value| value.fan_percent),
            fan_rpm: telemetry.as_ref().and_then(|value| value.fan_rpm),
        });
    }
}

fn nvml_query<T>(
    degraded: &mut bool,
    notes: &mut Option<&mut Vec<String>>,
    uuid: &str,
    metric: &str,
    result: Result<T, u32>,
) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(status) => {
            *degraded = true;
            report_issue(notes, || {
                format!("NVML GPU {uuid} {metric} unavailable: status {status}")
            });
            None
        }
    }
}
