use super::battery::*;
use super::device::*;
use super::storage::*;
use super::thermal::*;
use cclover_core::model::Collection;
use std::io;
use windows_sys::Win32::System::Ioctl::READ_ATTRIBUTE_BUFFER_SIZE;

use super::*;

#[test]
fn kelvin_tenths_conversion_matches_windows_units() {
    assert!((kelvin_tenths_to_celsius(3_001).unwrap() - 26.95).abs() < 1e-9);
}

#[test]
fn invalid_kelvin_temperature_is_rejected() {
    assert!(kelvin_tenths_to_celsius(0).is_err());
}

#[test]
fn storage_product_temperature_prefers_index_zero_even_when_unsorted() {
    let sensors = [
        StorageTemperature {
            index: 4,
            celsius: 44.0,
        },
        StorageTemperature {
            index: 0,
            celsius: 40.0,
        },
        StorageTemperature {
            index: 2,
            celsius: 42.0,
        },
    ];
    assert_eq!(primary_storage_temperature(&sensors), Some(&sensors[1]));
}

#[test]
fn storage_product_temperature_falls_back_to_lowest_index() {
    let sensors = [
        StorageTemperature {
            index: 3,
            celsius: 43.0,
        },
        StorageTemperature {
            index: 1,
            celsius: 41.0,
        },
        StorageTemperature {
            index: 2,
            celsius: 42.0,
        },
    ];
    assert_eq!(primary_storage_temperature(&sensors), Some(&sensors[1]));
}

#[test]
fn storage_product_temperature_handles_empty_sensor_set() {
    assert_eq!(primary_storage_temperature(&[]), None);
}

#[test]
fn ata_smart_temperature_prefers_attribute_194() {
    let mut page = [0_u8; READ_ATTRIBUTE_BUFFER_SIZE as usize];
    let airflow = ATA_SMART_ATTRIBUTE_OFFSET;
    page[airflow] = ATA_SMART_AIRFLOW_TEMPERATURE;
    page[airflow + 5] = 39;
    let primary = ATA_SMART_ATTRIBUTE_OFFSET + ATA_SMART_ATTRIBUTE_SIZE;
    page[primary] = ATA_SMART_TEMPERATURE;
    page[primary + 5] = 42;
    assert_eq!(ata_smart_temperature_from_page(&page), Some(42.0));
}

#[test]
fn ata_smart_temperature_falls_back_to_attribute_190() {
    let mut page = [0_u8; READ_ATTRIBUTE_BUFFER_SIZE as usize];
    page[ATA_SMART_ATTRIBUTE_OFFSET] = ATA_SMART_AIRFLOW_TEMPERATURE;
    page[ATA_SMART_ATTRIBUTE_OFFSET + 5] = 37;
    assert_eq!(ata_smart_temperature_from_page(&page), Some(37.0));
}

#[test]
fn ata_smart_temperature_rejects_implausible_raw_value() {
    let mut page = [0_u8; READ_ATTRIBUTE_BUFFER_SIZE as usize];
    page[ATA_SMART_ATTRIBUTE_OFFSET] = ATA_SMART_TEMPERATURE;
    page[ATA_SMART_ATTRIBUTE_OFFSET + 5] = 200;
    assert_eq!(ata_smart_temperature_from_page(&page), None);
}

fn storage_device(fallback_identity: bool) -> StorageDevice {
    StorageDevice {
        disk_number: 7,
        identity: "disk-id".to_owned(),
        fallback_identity,
    }
}

#[test]
fn storage_policy_selects_one_product_sensor_and_all_diagnostic_sensors() {
    let structured = StructuredTemperatureRead::Sensors(vec![
        StorageTemperature {
            index: 3,
            celsius: 43.0,
        },
        StorageTemperature {
            index: 0,
            celsius: 40.0,
        },
    ]);
    let device = storage_device(false);

    let (product, degraded) = project_storage_temperature(
        &device,
        StorageTemperatureProjection::Product,
        &structured,
        None,
    );
    assert!(!degraded);
    assert_eq!(product.len(), 1);
    assert_eq!(
        product[0].id.as_opaque_key(),
        "windows:storage:disk-id:temperature:0"
    );
    assert_eq!(product[0].celsius, 40.0);

    let (diagnostic, degraded) = project_storage_temperature(
        &device,
        StorageTemperatureProjection::Diagnostic,
        &structured,
        None,
    );
    assert!(!degraded);
    assert_eq!(diagnostic.len(), 2);
    assert_eq!(
        diagnostic[0].id.as_opaque_key(),
        "windows:storage:disk-id:temperature:3"
    );
    assert_eq!(
        diagnostic[1].id.as_opaque_key(),
        "windows:storage:disk-id:temperature:0"
    );
}

#[test]
fn storage_policy_uses_smart_for_unsupported_structured_temperature() {
    let device = storage_device(false);
    let (values, degraded) = project_storage_temperature(
        &device,
        StorageTemperatureProjection::Product,
        &StructuredTemperatureRead::Unsupported,
        Some(&SmartTemperatureRead::Temperature(38.0)),
    );
    assert!(!degraded);
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].id.as_opaque_key(),
        "windows:storage:disk-id:temperature:0"
    );
    assert_eq!(values[0].celsius, 38.0);
}

#[test]
fn storage_policy_distinguishes_expected_fallback_absence_from_hard_failure() {
    let device = storage_device(false);
    let (values, degraded) = project_storage_temperature(
        &device,
        StorageTemperatureProjection::Product,
        &StructuredTemperatureRead::Sensors(Vec::new()),
        Some(&SmartTemperatureRead::Unsupported),
    );
    assert!(values.is_empty());
    assert!(!degraded);

    let (values, degraded) = project_storage_temperature(
        &device,
        StorageTemperatureProjection::Product,
        &StructuredTemperatureRead::Failed(io::Error::other("structured failure")),
        Some(&SmartTemperatureRead::Empty),
    );
    assert!(values.is_empty());
    assert!(degraded);
}

#[test]
fn storage_policy_marks_locator_identity_as_degraded() {
    let device = storage_device(true);
    let (values, degraded) = project_storage_temperature(
        &device,
        StorageTemperatureProjection::Product,
        &StructuredTemperatureRead::Sensors(vec![StorageTemperature {
            index: 2,
            celsius: 42.0,
        }]),
        None,
    );
    assert_eq!(
        values[0].id.as_opaque_key(),
        "windows:storage:disk-id:temperature:2"
    );
    assert!(degraded);
}

#[test]
fn storage_native_results_preserve_capability_and_hard_failure_classes() {
    assert!(matches!(
        classify_structured_temperature(Err(io::Error::from_raw_os_error(ERROR_NOT_SUPPORTED))),
        StructuredTemperatureRead::Unsupported
    ));
    assert!(matches!(
        classify_structured_temperature(Err(io::Error::other("query failed"))),
        StructuredTemperatureRead::Failed(_)
    ));
    assert!(matches!(
        classify_smart_temperature(Err(io::Error::from_raw_os_error(ERROR_INVALID_FUNCTION))),
        SmartTemperatureRead::Unsupported
    ));
    assert!(matches!(
        classify_smart_temperature(Err(io::Error::other("SMART failed"))),
        SmartTemperatureRead::Failed(_)
    ));
}

#[test]
fn acpi_policy_keeps_partial_success_and_typed_identity() {
    let reads = vec![
        ("THERMAL-A".to_owned(), DeviceRead::Value(45.0)),
        ("THERMAL-B".to_owned(), DeviceRead::Unsupported),
        (
            "THERMAL-C".to_owned(),
            DeviceRead::Failed(io::Error::other("read failed")),
        ),
    ];
    let result = project_thermal_zones(&reads);
    let Collection::Degraded(values) = result else {
        panic!("hard device failure must degrade partial ACPI data");
    };
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].id.as_opaque_key(),
        "windows:acpi-thermal-zone:thermal-a"
    );
    assert_eq!(values[0].celsius, 45.0);
}

#[test]
fn acpi_unsupported_and_empty_are_successful_empty_observations() {
    let result = project_thermal_zones(&[
        ("a".to_owned(), DeviceRead::Unsupported),
        ("b".to_owned(), DeviceRead::Empty),
    ]);
    assert!(matches!(result, Collection::Available(values) if values.is_empty()));
}

#[test]
fn device_read_classification_keeps_unsupported_empty_and_invalid_distinct() {
    assert!(matches!(
        classify_device_read::<f64>(Ok(None)),
        DeviceRead::Empty
    ));
    assert!(matches!(
        classify_device_read::<f64>(Err(io::Error::from_raw_os_error(ERROR_NOT_SUPPORTED))),
        DeviceRead::Unsupported
    ));
    assert!(matches!(
        classify_device_read::<f64>(Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid temperature"
        ))),
        DeviceRead::Failed(_)
    ));
}

#[test]
fn battery_policy_keeps_partial_success_and_stable_identity() {
    let reads = vec![
        (
            "path-a".to_owned(),
            DeviceRead::Value(("battery-uid".to_owned(), 31.5)),
        ),
        ("path-b".to_owned(), DeviceRead::Unsupported),
        (
            "path-c".to_owned(),
            DeviceRead::Failed(io::Error::other("read failed")),
        ),
    ];
    let result = project_battery_temperatures(&reads);
    let Collection::Degraded(values) = result else {
        panic!("hard battery failure must degrade partial data");
    };
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].id.as_opaque_key(),
        "windows:battery:battery-uid:temperature"
    );
    assert_eq!(values[0].celsius, 31.5);
}

#[test]
fn battery_empty_and_unsupported_are_not_hard_failures() {
    let result = project_battery_temperatures(&[
        ("a".to_owned(), DeviceRead::Empty),
        ("b".to_owned(), DeviceRead::Unsupported),
    ]);
    assert!(matches!(result, Collection::Available(values) if values.is_empty()));

    let result = project_battery_temperatures(&[(
        "c".to_owned(),
        DeviceRead::Failed(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid battery temperature",
        )),
    )]);
    assert!(matches!(result, Collection::Degraded(values) if values.is_empty()));
}
