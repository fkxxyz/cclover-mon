mod hwmon;
mod nvml;

use std::collections::HashMap;
use std::time::Instant;

use crate::core::SAMPLE_INTERVAL;
use crate::core::model::TemperatureSnapshot;

use super::diagnostics::probe_note;

pub(super) struct Collector {
    hwmon: hwmon::Collector,
    nvml: nvml::Collector,
    last_sample: Option<Instant>,
    last_values: Vec<TemperatureSnapshot>,
}

impl Collector {
    pub(super) fn new() -> Self {
        Self {
            hwmon: hwmon::Collector::new(),
            nvml: nvml::Collector::new(),
            last_sample: None,
            last_values: Vec::new(),
        }
    }

    pub(super) fn collect(
        &mut self,
        now: Instant,
        mut notes: Option<&mut Vec<String>>,
    ) -> Vec<TemperatureSnapshot> {
        if self
            .last_sample
            .is_some_and(|last| now.saturating_duration_since(last) < SAMPLE_INTERVAL)
        {
            probe_note(&mut notes, || {
                "temperature sample skipped: cached values are still fresh".to_owned()
            });
            return self.last_values.clone();
        }
        self.last_sample = Some(now);

        let mut values = self.hwmon.collect(now, notes.as_deref_mut());
        values.extend(self.nvml.collect(notes));
        normalize_display_names(&mut values);

        self.last_values = values.clone();
        values
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
