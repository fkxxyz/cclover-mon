mod runtime;

use crate::core::model::TemperatureSnapshot;

use super::super::diagnostics::{probe_note, report_issue};

pub(super) struct Collector {
    state: State,
}

enum State {
    Uninitialized,
    Available(runtime::Session),
    Unavailable(String),
}

impl Collector {
    pub(super) fn new() -> Self {
        Self {
            state: State::Uninitialized,
        }
    }

    pub(super) fn collect(
        &mut self,
        mut notes: Option<&mut Vec<String>>,
    ) -> Vec<TemperatureSnapshot> {
        if matches!(self.state, State::Uninitialized) {
            self.state = match runtime::Session::load() {
                Ok((session, issues)) => {
                    for issue in issues {
                        report_issue(&mut notes, || issue.clone());
                    }
                    State::Available(session)
                }
                Err(reason) => State::Unavailable(reason),
            };
        }

        match &self.state {
            State::Uninitialized => unreachable!(),
            State::Unavailable(reason) => {
                probe_note(&mut notes, || format!("NVML unavailable: {reason}"));
                Vec::new()
            }
            State::Available(session) => collect_session(session, notes),
        }
    }
}

fn collect_session(
    session: &runtime::Session,
    mut notes: Option<&mut Vec<String>>,
) -> Vec<TemperatureSnapshot> {
    let mut values = Vec::with_capacity(session.devices().len());
    for (index, device) in session.devices().iter().enumerate() {
        let temperature = match session.temperature(index) {
            Ok(temperature) => temperature,
            Err(status) => {
                probe_note(&mut notes, || {
                    format!(
                        "NVML GPU {} skipped: temperature query failed with status {status}",
                        device.uuid()
                    )
                });
                continue;
            }
        };

        values.push(snapshot(device.uuid(), device.name(), temperature));
    }
    values
}

fn snapshot(uuid: &str, name: &str, temperature: u32) -> TemperatureSnapshot {
    TemperatureSnapshot {
        id: format!("nvml:{uuid}"),
        name: name.to_owned(),
        celsius: f64::from(temperature),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nvml_identity_is_namespaced() {
        let value = snapshot("GPU-test", "NVIDIA Test Device", 51);
        assert_eq!(value.id, "nvml:GPU-test");
        assert_eq!(value.name, "NVIDIA Test Device");
        assert_eq!(value.celsius, 51.0);
    }
}
