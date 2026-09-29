mod hwmon;

use std::collections::HashMap;
use std::time::Instant;

use crate::core::SAMPLE_INTERVAL;
use crate::core::model::{Collection, TemperatureSnapshot};

use super::diagnostics::probe_note;

pub(super) struct Collector {
    hwmon: hwmon::Collector,
    last_sample: Option<Instant>,
    last_outcome: Collection<Vec<TemperatureSnapshot>>,
}

impl Collector {
    pub(super) fn new() -> Self {
        Self {
            hwmon: hwmon::Collector::new(),
            last_sample: None,
            last_outcome: Collection::default(),
        }
    }

    pub(super) fn collect(
        &mut self,
        now: Instant,
        nvml: Collection<Vec<TemperatureSnapshot>>,
        mut notes: Option<&mut Vec<String>>,
    ) -> Collection<Vec<TemperatureSnapshot>> {
        if self
            .last_sample
            .is_some_and(|last| now.saturating_duration_since(last) < SAMPLE_INTERVAL)
        {
            probe_note(&mut notes, || {
                "temperature sample skipped: cached values are still fresh".to_owned()
            });
            return self.last_outcome.clone();
        }
        self.last_sample = Some(now);

        let hwmon = self.hwmon.collect(now, notes);
        let mut outcome = merge_sources(hwmon, nvml);
        if let Some(values) = outcome_value_mut(&mut outcome) {
            normalize_display_names(values);
        }

        self.last_outcome = outcome.clone();
        outcome
    }
}

fn outcome_value_mut<T>(outcome: &mut Collection<T>) -> Option<&mut T> {
    match outcome {
        Collection::Available(value) | Collection::Degraded(value) => Some(value),
        Collection::Unavailable(_) => None,
    }
}

fn merge_sources<T>(left: Collection<Vec<T>>, right: Collection<Vec<T>>) -> Collection<Vec<T>> {
    use Collection::{Available, Degraded, Unavailable};

    match (left, right) {
        (Available(mut left), Available(right)) => {
            left.extend(right);
            Available(left)
        }
        (Available(mut left), Degraded(right))
        | (Degraded(mut left), Available(right))
        | (Degraded(mut left), Degraded(right)) => {
            left.extend(right);
            Degraded(left)
        }
        (Available(value), Unavailable(_))
        | (Degraded(value), Unavailable(_))
        | (Unavailable(_), Available(value))
        | (Unavailable(_), Degraded(value)) => Degraded(value),
        (Unavailable(reason), Unavailable(_)) => Unavailable(reason),
    }
}

fn normalize_display_names(values: &mut [TemperatureSnapshot]) {
    let mut counts = HashMap::new();
    for value in values.iter() {
        *counts.entry(value.name.clone()).or_insert(0_usize) += 1;
    }

    let mut ordinals = HashMap::new();
    for value in values.iter_mut() {
        if counts.get(&value.name).copied().unwrap_or(0) <= 1 {
            continue;
        }

        let ordinal = ordinals.entry(value.name.clone()).or_insert(0_usize);
        *ordinal += 1;
        value.name = format!("{} {}", value.name, ordinal);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_display_names_are_numbered_without_changing_order_or_identity() {
        let mut values = vec![
            TemperatureSnapshot {
                id: "nvml:GPU-b".to_owned(),
                name: "GPU".to_owned(),
                celsius: 51.0,
            },
            TemperatureSnapshot {
                id: "hwmon:pci-a:temp1".to_owned(),
                name: "GPU".to_owned(),
                celsius: 49.0,
            },
            TemperatureSnapshot {
                id: "hwmon:cpu:temp1".to_owned(),
                name: "CPU".to_owned(),
                celsius: 61.0,
            },
        ];

        normalize_display_names(&mut values);

        assert_eq!(values[0].name, "GPU 1");
        assert_eq!(values[0].id, "nvml:GPU-b");
        assert_eq!(values[1].name, "GPU 2");
        assert_eq!(values[1].id, "hwmon:pci-a:temp1");
        assert_eq!(values[2].name, "CPU");
        assert_eq!(values[2].id, "hwmon:cpu:temp1");
    }
}
