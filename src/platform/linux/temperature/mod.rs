mod hwmon;

use std::collections::HashMap;
use std::time::Instant;

use crate::core::SAMPLE_INTERVAL;
use crate::core::model::{Collection, TemperatureSnapshot};

use super::PhysicalDeviceId;
use super::diagnostics::probe_note;

#[derive(Clone, Debug)]
pub(super) struct Observation {
    pub(super) physical_device: Option<PhysicalDeviceId>,
    pub(super) snapshot: TemperatureSnapshot,
}

pub(super) struct Collector {
    hwmon: hwmon::Collector,
    last_sample: Option<Instant>,
    last_outcome: Collection<Vec<Observation>>,
}

impl Collector {
    pub(super) fn new() -> Self {
        Self {
            hwmon: hwmon::Collector::new(),
            last_sample: None,
            last_outcome: Collection::default(),
        }
    }

    pub(super) fn collect_observations(
        &mut self,
        now: Instant,
        mut notes: Option<&mut Vec<String>>,
    ) -> Collection<Vec<Observation>> {
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

        let outcome = self.hwmon.collect(now, notes);
        self.last_outcome = outcome.clone();
        outcome
    }
}

pub(super) fn normalize_display_names(values: &mut [TemperatureSnapshot]) {
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
    use crate::core::model::TemperatureId;

    #[test]
    fn duplicate_display_names_are_numbered_without_changing_order_or_identity() {
        let mut values = vec![
            TemperatureSnapshot {
                id: TemperatureId::from_opaque_key("nvml:GPU-b"),
                name: "GPU".to_owned(),
                celsius: 51.0,
            },
            TemperatureSnapshot {
                id: TemperatureId::from_opaque_key("hwmon:pci-a:temp1"),
                name: "GPU".to_owned(),
                celsius: 49.0,
            },
            TemperatureSnapshot {
                id: TemperatureId::from_opaque_key("hwmon:cpu:temp1"),
                name: "CPU".to_owned(),
                celsius: 61.0,
            },
        ];

        normalize_display_names(&mut values);

        assert_eq!(values[0].name, "GPU 1");
        assert_eq!(values[0].id.as_opaque_key(), "nvml:GPU-b");
        assert_eq!(values[1].name, "GPU 2");
        assert_eq!(values[1].id.as_opaque_key(), "hwmon:pci-a:temp1");
        assert_eq!(values[2].name, "CPU");
        assert_eq!(values[2].id.as_opaque_key(), "hwmon:cpu:temp1");
    }
}
