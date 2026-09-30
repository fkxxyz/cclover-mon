use crate::core::model::{Collection, CollectionUnavailable, GpuId, GpuSnapshot};

use super::diagnostics::report_issue;
use super::gpu_d3dkmt::VendorPresence;
use super::{gpu_adl, gpu_d3dkmt, gpu_nvml};

pub(super) struct Collector {
    nvml: Source<gpu_nvml::Session>,
    adl: Source<gpu_adl::Session>,
    d3dkmt: Source<gpu_d3dkmt::Session>,
}

enum Source<T> {
    Uninitialized,
    Available { session: T, degraded: bool },
    Unsupported(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BackendPlan {
    nvml: bool,
    adl: bool,
    d3dkmt: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BackendAvailability {
    nvml: bool,
    adl: bool,
    d3dkmt: bool,
}

impl Collector {
    pub(super) fn new() -> Self {
        Self {
            nvml: Source::Uninitialized,
            adl: Source::Uninitialized,
            d3dkmt: Source::Uninitialized,
        }
    }

    pub(super) fn collect(
        &mut self,
        mut notes: Option<&mut Vec<String>>,
    ) -> Collection<Vec<GpuSnapshot>> {
        initialize_source(
            &mut self.d3dkmt,
            gpu_d3dkmt::Session::load,
            "D3DKMT",
            &mut notes,
        );

        let topology = match &self.d3dkmt {
            Source::Available { session, .. } => Some(session.vendors()),
            _ => None,
        };
        let plan = match topology {
            Some(vendors) => backend_plan(vendors.nvidia(), vendors.amd(), vendors.intel()),
            None => backend_plan(
                VendorPresence::Unknown,
                VendorPresence::Unknown,
                VendorPresence::Unknown,
            ),
        };
        if plan.nvml {
            initialize_source(&mut self.nvml, gpu_nvml::Session::load, "NVML", &mut notes);
        }
        if plan.adl {
            initialize_source(&mut self.adl, gpu_adl::Session::load, "ADL", &mut notes);
        }

        let availability = BackendAvailability {
            nvml: matches!(self.nvml, Source::Available { .. }),
            adl: matches!(self.adl, Source::Available { .. }),
            d3dkmt: matches!(self.d3dkmt, Source::Available { .. }),
        };
        let mut degraded =
            match initial_collection_degradation(topology.is_some(), plan, availability) {
                Ok(degraded) => degraded,
                Err(reason) => {
                    report_unsupported(&self.nvml, "NVML", &mut notes);
                    report_unsupported(&self.adl, "ADL", &mut notes);
                    report_unsupported(&self.d3dkmt, "D3DKMT", &mut notes);
                    return Collection::unavailable(reason);
                }
            };

        let mut values = Vec::new();
        if plan.nvml {
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
        }
        if plan.adl {
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
        }
        if plan.d3dkmt {
            if let Source::Available {
                session,
                degraded: source_degraded,
            } = &self.d3dkmt
            {
                degraded |= source_degraded;
                collect_d3dkmt(session, &mut values, &mut degraded, &mut notes);
            } else {
                report_unsupported(&self.d3dkmt, "D3DKMT", &mut notes);
            }
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

fn backend_plan(nvidia: VendorPresence, amd: VendorPresence, intel: VendorPresence) -> BackendPlan {
    BackendPlan {
        nvml: nvidia != VendorPresence::Absent,
        adl: amd != VendorPresence::Absent,
        d3dkmt: intel != VendorPresence::Absent,
    }
}

fn initial_collection_degradation(
    topology_known: bool,
    plan: BackendPlan,
    availability: BackendAvailability,
) -> Result<bool, CollectionUnavailable> {
    if topology_known && !plan.nvml && !plan.adl && !plan.d3dkmt {
        return Err(CollectionUnavailable::Unsupported);
    }
    if !availability.nvml && !availability.adl && !availability.d3dkmt {
        return Err(CollectionUnavailable::Unsupported);
    }
    Ok(!topology_known || (plan.nvml && !availability.nvml) || (plan.adl && !availability.adl))
}

fn query_value<T, E>(degraded: &mut bool, result: Result<T, E>) -> (Option<T>, Option<E>) {
    match result {
        Ok(value) => (Some(value), None),
        Err(error) => {
            *degraded = true;
            (None, Some(error))
        }
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
        let (memory_used_bytes, memory_error) =
            query_value(degraded, session.memory_used_bytes(index));
        if let Some(status) = memory_error {
            report_issue(notes, || {
                format!(
                    "ADL GPU {} dedicated VRAM usage unavailable: status {status}",
                    device.stable_key()
                )
            });
        }
        let memory_used_bytes = memory_used_bytes.flatten();
        let (telemetry, telemetry_error) = query_value(degraded, session.telemetry(index));
        if let Some(status) = telemetry_error {
            report_issue(notes, || {
                format!(
                    "ADL GPU {} telemetry unavailable: status {status}",
                    device.stable_key()
                )
            });
        }
        let telemetry = telemetry.flatten();
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

fn collect_d3dkmt(
    session: &gpu_d3dkmt::Session,
    values: &mut Vec<GpuSnapshot>,
    degraded: &mut bool,
    notes: &mut Option<&mut Vec<String>>,
) {
    values.reserve(session.devices().len());
    for (index, device) in session.devices().iter().enumerate() {
        let (temperature_celsius, temperature_error) =
            query_value(degraded, session.temperature(index));
        if let Some(status) = temperature_error {
            report_issue(notes, || {
                format!(
                    "D3DKMT Intel GPU {} temperature unavailable: status {status}",
                    device.stable_key()
                )
            });
        }
        values.push(GpuSnapshot {
            id: GpuId::from_opaque_key(format!("d3dkmt:{}", device.stable_key())),
            name: device.name().to_owned(),
            utilization_percent: None,
            memory_used_bytes: None,
            memory_total_bytes: None,
            temperature_celsius,
            power_watts: None,
            core_clock_mhz: None,
            fan_percent: None,
            fan_rpm: None,
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
    let (value, error) = query_value(degraded, result);
    if let Some(status) = error {
        report_issue(notes, || {
            format!("NVML GPU {uuid} {metric} unavailable: status {status}")
        });
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    fn availability(nvml: bool, adl: bool, d3dkmt: bool) -> BackendAvailability {
        BackendAvailability { nvml, adl, d3dkmt }
    }

    #[test]
    fn backend_plan_distinguishes_present_absent_and_unknown_vendors() {
        assert_eq!(
            backend_plan(
                VendorPresence::Present,
                VendorPresence::Absent,
                VendorPresence::Unknown,
            ),
            BackendPlan {
                nvml: true,
                adl: false,
                d3dkmt: true,
            }
        );
        assert_eq!(
            backend_plan(
                VendorPresence::Unknown,
                VendorPresence::Unknown,
                VendorPresence::Unknown,
            ),
            BackendPlan {
                nvml: true,
                adl: true,
                d3dkmt: true,
            }
        );
        assert_eq!(
            backend_plan(
                VendorPresence::Absent,
                VendorPresence::Absent,
                VendorPresence::Absent,
            ),
            BackendPlan {
                nvml: false,
                adl: false,
                d3dkmt: false,
            }
        );
    }

    #[test]
    fn known_topology_with_only_expected_backend_available_is_not_degraded() {
        let plan = backend_plan(
            VendorPresence::Absent,
            VendorPresence::Absent,
            VendorPresence::Present,
        );
        assert_eq!(
            initial_collection_degradation(true, plan, availability(false, false, true)),
            Ok(false)
        );
    }

    #[test]
    fn unknown_topology_or_missing_expected_vendor_backend_is_degraded() {
        let unknown = backend_plan(
            VendorPresence::Unknown,
            VendorPresence::Unknown,
            VendorPresence::Unknown,
        );
        assert_eq!(
            initial_collection_degradation(false, unknown, availability(true, false, false)),
            Ok(true)
        );

        let nvidia = backend_plan(
            VendorPresence::Present,
            VendorPresence::Absent,
            VendorPresence::Present,
        );
        assert_eq!(
            initial_collection_degradation(true, nvidia, availability(false, false, true)),
            Ok(true)
        );
    }

    #[test]
    fn no_applicable_or_no_available_gpu_backend_is_unsupported() {
        let absent = backend_plan(
            VendorPresence::Absent,
            VendorPresence::Absent,
            VendorPresence::Absent,
        );
        assert_eq!(
            initial_collection_degradation(true, absent, availability(false, false, true)),
            Err(CollectionUnavailable::Unsupported)
        );

        let expected = backend_plan(
            VendorPresence::Present,
            VendorPresence::Absent,
            VendorPresence::Absent,
        );
        assert_eq!(
            initial_collection_degradation(true, expected, availability(false, false, false)),
            Err(CollectionUnavailable::Unsupported)
        );
    }

    #[test]
    fn backend_load_outcomes_are_deterministic_without_vendor_runtime() {
        let mut notes = None;
        let mut available = Source::Uninitialized;
        initialize_source(
            &mut available,
            || Ok::<_, String>((7_u8, Vec::new())),
            "test",
            &mut notes,
        );
        assert!(matches!(
            available,
            Source::Available {
                session: 7,
                degraded: false
            }
        ));

        let mut degraded = Source::Uninitialized;
        initialize_source(
            &mut degraded,
            || Ok::<_, String>((9_u8, vec!["partial discovery".to_owned()])),
            "test",
            &mut notes,
        );
        assert!(matches!(
            degraded,
            Source::Available {
                session: 9,
                degraded: true
            }
        ));

        let mut unsupported = Source::<u8>::Uninitialized;
        initialize_source(
            &mut unsupported,
            || Err("runtime missing".to_owned()),
            "test",
            &mut notes,
        );
        assert!(matches!(unsupported, Source::Unsupported(reason) if reason == "runtime missing"));
    }

    #[test]
    fn backend_query_failure_preserves_partial_collection_and_marks_degraded() {
        let mut degraded = false;
        let (value, error) = query_value(&mut degraded, Ok::<_, i32>(57.0));
        assert_eq!(value, Some(57.0));
        assert_eq!(error, None);
        assert!(!degraded);

        let (value, error) = query_value::<f64, _>(&mut degraded, Err(-5_i32));
        assert_eq!(value, None);
        assert_eq!(error, Some(-5));
        assert!(degraded);
    }
}
