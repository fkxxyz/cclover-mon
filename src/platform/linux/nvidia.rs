mod runtime;

use crate::core::model::{Collection, CollectionUnavailable, GpuId, GpuSnapshot};

use super::diagnostics::{probe_note, report_issue};

pub(super) struct Collector {
    state: State,
}

enum State {
    Uninitialized,
    Available {
        session: runtime::Session,
        degraded: bool,
    },
    Unavailable(String),
}

impl Collector {
    pub(super) fn new() -> Self {
        Self {
            state: State::Uninitialized,
        }
    }

    fn ensure_initialized(&mut self, notes: &mut Option<&mut Vec<String>>) {
        if !matches!(self.state, State::Uninitialized) {
            return;
        }

        self.state = match runtime::Session::load() {
            Ok((session, issues)) => {
                let degraded = !issues.is_empty();
                for issue in issues {
                    report_issue(notes, || issue.clone());
                }
                State::Available { session, degraded }
            }
            Err(reason) => State::Unavailable(reason),
        };
    }

    pub(super) fn gpus(
        &mut self,
        mut notes: Option<&mut Vec<String>>,
    ) -> Collection<Vec<GpuSnapshot>> {
        self.ensure_initialized(&mut notes);
        match &self.state {
            State::Uninitialized => unreachable!(),
            State::Unavailable(reason) => {
                probe_note(&mut notes, || format!("NVML unavailable: {reason}"));
                Collection::unavailable(CollectionUnavailable::Unsupported)
            }
            State::Available { session, degraded } => {
                let mut degraded = *degraded;
                let mut values = Vec::with_capacity(session.devices().len());
                for (index, device) in session.devices().iter().enumerate() {
                    let utilization_percent = optional_query(
                        &mut degraded,
                        &mut notes,
                        device.uuid(),
                        "utilization",
                        session.utilization(index).map(|value| f64::from(value.gpu)),
                    );
                    let memory = optional_query(
                        &mut degraded,
                        &mut notes,
                        device.uuid(),
                        "memory",
                        session.memory(index),
                    );
                    let temperature_celsius = optional_query(
                        &mut degraded,
                        &mut notes,
                        device.uuid(),
                        "temperature",
                        session.temperature(index).map(f64::from),
                    );
                    let power_watts = optional_query(
                        &mut degraded,
                        &mut notes,
                        device.uuid(),
                        "power",
                        session
                            .power_milliwatts(index)
                            .map(|value| f64::from(value) / 1000.0),
                    );
                    let core_clock_mhz = optional_query(
                        &mut degraded,
                        &mut notes,
                        device.uuid(),
                        "graphics clock",
                        session.graphics_clock_mhz(index).map(u64::from),
                    );
                    let fan_percent = optional_query(
                        &mut degraded,
                        &mut notes,
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
                if degraded {
                    Collection::degraded(values)
                } else {
                    Collection::available(values)
                }
            }
        }
    }
}

fn optional_query<T>(
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
            probe_note(notes, || {
                format!("NVML GPU {uuid} {metric} unavailable: status {status}")
            });
            None
        }
    }
}
