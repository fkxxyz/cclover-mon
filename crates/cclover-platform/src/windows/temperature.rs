#![allow(unsafe_code)]

use windows_sys::Win32::System::Power::{GUID_DEVICE_BATTERY, GUID_DEVICE_THERMAL_ZONE};

use cclover_core::model::{Collection, TemperatureSnapshot};

use super::diagnostics::report_issue;
use super::hardware;
use device::device_interface_paths;
use storage::{StorageDevice, StorageTemperatureProjection};

mod battery;
mod device;
mod storage;
mod thermal;

pub(super) struct Collector {
    storage: Vec<StorageDevice>,
    thermal_paths: Vec<String>,
    battery_paths: Vec<String>,
    discovery_issues: Vec<String>,
}

impl Collector {
    pub(super) fn new() -> Self {
        let mut discovery_issues = Vec::new();
        let storage = storage::discover_storage_devices(&mut discovery_issues);
        let thermal_paths =
            device_interface_paths(&GUID_DEVICE_THERMAL_ZONE).unwrap_or_else(|error| {
                discovery_issues.push(format!("ACPI thermal-zone discovery failed: {error}"));
                Vec::new()
            });
        let battery_paths = device_interface_paths(&GUID_DEVICE_BATTERY).unwrap_or_else(|error| {
            discovery_issues.push(format!("battery discovery failed: {error}"));
            Vec::new()
        });
        Self {
            storage,
            thermal_paths,
            battery_paths,
            discovery_issues,
        }
    }

    pub(super) fn collect(
        &self,
        notes: Option<&mut Vec<String>>,
    ) -> Collection<Vec<TemperatureSnapshot>> {
        self.collect_with_storage_projection(StorageTemperatureProjection::Product, notes)
    }

    pub(super) fn collect_diagnostic(
        &self,
        notes: Option<&mut Vec<String>>,
    ) -> Collection<Vec<TemperatureSnapshot>> {
        self.collect_with_storage_projection(StorageTemperatureProjection::Diagnostic, notes)
    }

    fn collect_with_storage_projection(
        &self,
        projection: StorageTemperatureProjection,
        mut notes: Option<&mut Vec<String>>,
    ) -> Collection<Vec<TemperatureSnapshot>> {
        for issue in &self.discovery_issues {
            report_issue(&mut notes, || issue.clone());
        }
        let storage =
            storage::collect_storage_temperatures(&self.storage, projection, notes.as_deref_mut());
        let thermal = thermal::collect_thermal_zones(&self.thermal_paths, notes.as_deref_mut());
        let battery = battery::collect_battery_temperatures(&self.battery_paths, notes);
        let merged = hardware::merge_temperature_sources([storage, thermal, battery]);
        if self.discovery_issues.is_empty() {
            merged
        } else {
            match merged {
                Collection::Available(values) => Collection::degraded(values),
                other => other,
            }
        }
    }
}

impl Default for Collector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
