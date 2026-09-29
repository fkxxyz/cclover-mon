mod runtime;

use crate::core::model::{
    Collection, CollectionUnavailable, GpuMemorySnapshot, TemperatureSnapshot,
};

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

    pub(super) fn temperatures(
        &mut self,
        mut notes: Option<&mut Vec<String>>,
    ) -> Collection<Vec<TemperatureSnapshot>> {
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
                    let temperature = match session.temperature(index) {
                        Ok(value) => value,
                        Err(status) => {
                            degraded = true;
                            probe_note(&mut notes, || {
                                format!(
                                    "NVML GPU {} skipped: temperature query failed with status {status}",
                                    device.uuid()
                                )
                            });
                            continue;
                        }
                    };
                    values.push(TemperatureSnapshot {
                        id: format!("nvml:{}", device.uuid()),
                        name: device.name().to_owned(),
                        celsius: f64::from(temperature),
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

    pub(super) fn memory(
        &mut self,
        mut notes: Option<&mut Vec<String>>,
    ) -> Collection<Vec<GpuMemorySnapshot>> {
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
                    let memory = match session.memory(index) {
                        Ok(value) => value,
                        Err(status) => {
                            degraded = true;
                            probe_note(&mut notes, || {
                                format!(
                                    "NVML GPU {} skipped: memory query failed with status {status}",
                                    device.uuid()
                                )
                            });
                            continue;
                        }
                    };
                    values.push(GpuMemorySnapshot {
                        id: crate::core::model::GpuId::from_opaque_key(format!(
                            "nvml:{}",
                            device.uuid()
                        )),
                        name: device.name().to_owned(),
                        used_bytes: memory.used,
                        total_bytes: memory.total,
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
