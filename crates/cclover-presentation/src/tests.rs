use super::*;
use cclover_core::model::DiskMetadata;

#[test]
fn disk_title_prefers_user_recognizable_associated_labels() {
    assert_eq!(disk_title("Disk 0", &[]), "Disk 0");
    assert_eq!(disk_title("Disk 0", &["C:".into()]), "C:");
    assert_eq!(disk_title("Disk 0", &["C:".into(), "D:".into()]), "C: · D:");
    assert_eq!(
        disk_title(
            "Disk 0",
            &["C:".into(), "D:".into(), "E:".into(), "F:".into()]
        ),
        "C: · D: +2"
    );
}

#[test]
fn shared_value_formatting_is_stable() {
    assert_eq!(format_bytes(999), "999 B");
    assert_eq!(format_bytes(1024), "1.00 KiB");
    assert_eq!(format_bytes(10 * 1024), "10.0 KiB");
    assert_eq!(format_rate(1024.0), "1.00 KiB/s");
    assert_eq!(format_compact_rate(1024.0).as_str(), "1.0K/s");
    assert_eq!(format_percent(12.34), "12.3%");
    assert_eq!(unavailable(), "—");
}

#[test]
fn bounded_dashboard_formatters_hold_their_declared_capacity() {
    use crate::format::{
        format_compact_bytes, format_compact_bytes_pair, format_compact_frequency_mhz,
        format_compact_percent, format_compact_power, format_compact_rpm,
        format_compact_temperature, format_gpu_memory,
    };

    let values = [0, 999, 1024, 1024 * 1024, u64::MAX];
    for value in values {
        assert_within_bound(&format_compact_bytes(value));
        assert_within_bound(&format_compact_frequency_mhz(value));
        assert_within_bound(&format_compact_rpm(value));
    }
    for (used, total) in [
        (0, 0),
        (999, 1024),
        (4 << 30, 22 << 30),
        (u64::MAX, u64::MAX),
    ] {
        assert_within_bound(&format_compact_bytes_pair(used, total));
        assert_within_bound(&format_gpu_memory(used, total));
    }
    for value in [0.0, 42.0, 100.0, 1000.0, f64::MAX, f64::INFINITY, f64::NAN] {
        assert_within_bound(&format_compact_rate(value));
        assert_within_bound(&format_compact_percent(value));
        assert_within_bound(&format_compact_power(value));
        assert_within_bound(&format_compact_temperature(value));
    }
}

fn assert_within_bound(value: &BoundedText) {
    assert!(value.as_str().chars().count() <= usize::from(value.max_columns()));
}

#[test]
fn gpu_panel_formats_public_metrics_and_histories() {
    let mut state = MonitorState::default();
    let gpu_id = cclover_core::model::GpuId::from_opaque_key("gpu-a");
    state.snapshot.gpus = Collection::available(vec![GpuSnapshot {
        id: gpu_id.clone(),
        name: "NVIDIA GeForce RTX Test".into(),
        utilization_percent: Some(42.0),
        memory_used_bytes: Some(4 * 1024 * 1024 * 1024),
        memory_total_bytes: Some(8 * 1024 * 1024 * 1024),
        temperature_celsius: Some(63.0),
        power_watts: Some(145.0),
        core_clock_mhz: Some(1830),
        fan_percent: Some(37.0),
        fan_rpm: Some(1320),
    }]);
    state
        .history
        .gpu_utilization
        .insert(gpu_id.clone(), VecDeque::from([40.0, 42.0]));
    state
        .history
        .gpu_memory_used
        .insert(gpu_id.clone(), VecDeque::from([1.0, 2.0]));
    state
        .history
        .gpu_temperature
        .insert(gpu_id, VecDeque::from([61.0, 63.0]));

    let panel = Dashboard::new(&state).gpu(0).unwrap();
    assert_eq!(panel.name(), "RTX Test");
    assert_eq!(panel.utilization_value(), "42.0%");
    assert_eq!(panel.memory_value(), "4.00 GiB / 8.00 GiB");
    assert_eq!(panel.temperature_value(), "63.0°C");
    assert_eq!(panel.power_value(), "145 W");
    assert_eq!(panel.core_clock_value(), "1830 MHz");
    assert_eq!(panel.fan_value(), "37.0%");
    assert_eq!(panel.compact_utilization_value().as_str(), "42.0%");
    assert_eq!(panel.compact_memory_value().as_str(), "4.00 GiB / 8 GiB");
    assert_eq!(panel.compact_temperature_value().as_str(), "63.0°C");
    assert_eq!(panel.compact_power_value().as_str(), "145W");
    assert_eq!(panel.compact_core_clock_value().as_str(), "1.8GHz");
    assert_eq!(panel.compact_fan_value().as_str(), "37.0%");
    assert_eq!(
        panel.utilization_history().unwrap(),
        &VecDeque::from([40.0, 42.0])
    );
    assert_eq!(panel.memory_history().unwrap(), &VecDeque::from([1.0, 2.0]));
    assert_eq!(
        panel.temperature_history().unwrap(),
        &VecDeque::from([61.0, 63.0])
    );
    assert_eq!(
        panel.memory_graph_max(),
        (8_u64 * 1024 * 1024 * 1024) as f64
    );
}

#[test]
fn gpu_memory_keeps_readable_precision_and_units() {
    use crate::format::format_gpu_memory;

    let gib = 1024_u64.pow(3);
    let used = (11.52_f64 * gib as f64).round() as u64;
    let value = format_gpu_memory(used, 22 * gib);

    assert_eq!(value.as_str(), "11.52 GiB / 22 GiB");
    assert_eq!(value.max_columns(), 20);
}

#[test]
fn unavailable_attribution_does_not_hide_available_parent_metric() {
    let disk_id = cclover_core::model::DiskId::from_opaque_key("disk-a");
    let mut state = MonitorState::default();
    state.snapshot.disks = Collection::available(vec![DiskSnapshot {
        id: disk_id,
        metadata: DiskMetadata {
            system_label: "nvme0n1".into(),
            associated_labels: Vec::new(),
        },
        bytes_per_sec: 10.0,
    }]);
    state.snapshot.process_disk_io =
        Collection::unavailable(cclover_core::model::CollectionUnavailable::PermissionDenied);

    let panel = Dashboard::new(&state).disk(0).unwrap();
    assert_eq!(panel.name(), "nvme0n1");
    assert!(!panel.process_rows_visible());
    assert_eq!(panel.processes().count(), 0);

    state.snapshot.process_disk_io = Collection::available(Vec::new());
    let panel = Dashboard::new(&state).disk(0).unwrap();
    assert!(panel.process_rows_visible());
    assert_eq!(panel.processes().count(), 0);
}

#[test]
fn io_process_rows_match_stable_device_identity_and_hide_zero_rate() {
    let disk_id = cclover_core::model::DiskId::from_opaque_key("disk-a");
    let other_disk_id = cclover_core::model::DiskId::from_opaque_key("disk-b");
    let network_id = cclover_core::model::NetworkId::from_opaque_key("network-a");
    let other_network_id = cclover_core::model::NetworkId::from_opaque_key("network-b");
    let process = cclover_core::model::ProcessInstanceId {
        pid: 42,
        birth_marker: 7,
    };
    let mut state = MonitorState::default();
    state.snapshot.disks = Collection::available(vec![DiskSnapshot {
        id: disk_id.clone(),
        metadata: DiskMetadata {
            system_label: "nvme0n1".into(),
            associated_labels: Vec::new(),
        },
        bytes_per_sec: 0.0,
    }]);
    state.snapshot.networks = Collection::available(vec![NetworkSnapshot {
        id: network_id.clone(),
        name: "eth0".into(),
        down_bytes_per_sec: 0.0,
        up_bytes_per_sec: 0.0,
    }]);
    state.snapshot.process_disk_io = Collection::available(vec![
        ProcessDiskIo {
            process,
            name: Some("worker".into()),
            disk_id: disk_id.clone(),
            device: "renamed-display-label".into(),
            read_bytes_per_sec: 2048.0,
            write_bytes_per_sec: 1024.0,
        },
        ProcessDiskIo {
            process,
            name: Some("idle".into()),
            disk_id,
            device: "nvme0n1".into(),
            read_bytes_per_sec: 0.0,
            write_bytes_per_sec: 0.0,
        },
        ProcessDiskIo {
            process,
            name: Some("other".into()),
            disk_id: other_disk_id,
            device: "nvme1n1".into(),
            read_bytes_per_sec: 4096.0,
            write_bytes_per_sec: 4096.0,
        },
    ]);
    state.snapshot.process_network_io = Collection::available(vec![
        ProcessNetworkIo {
            process,
            name: Some("worker".into()),
            network_id: network_id.clone(),
            interface: "renamed-display-label".into(),
            rx_bytes_per_sec: 3072.0,
            tx_bytes_per_sec: 1024.0,
        },
        ProcessNetworkIo {
            process,
            name: Some("idle".into()),
            network_id,
            interface: "eth0".into(),
            rx_bytes_per_sec: 0.0,
            tx_bytes_per_sec: 0.0,
        },
        ProcessNetworkIo {
            process,
            name: Some("other".into()),
            network_id: other_network_id,
            interface: "eth1".into(),
            rx_bytes_per_sec: 4096.0,
            tx_bytes_per_sec: 4096.0,
        },
    ]);

    let disk_rows: Vec<_> = Dashboard::new(&state)
        .disk(0)
        .unwrap()
        .processes()
        .collect();
    let network_rows: Vec<_> = Dashboard::new(&state)
        .network(0)
        .unwrap()
        .processes()
        .collect();

    assert_eq!(disk_rows.len(), 1);
    assert_eq!(disk_rows[0].name, Some("worker"));
    assert_eq!(disk_rows[0].pid, 42);
    assert_eq!(disk_rows[0].first_value, "2.00 KiB/s");
    assert_eq!(disk_rows[0].second_value, "1.00 KiB/s");
    assert_eq!(disk_rows[0].compact_first_value.as_str(), "2.0K/s");
    assert_eq!(disk_rows[0].compact_second_value.as_str(), "1.0K/s");
    assert_eq!(network_rows.len(), 1);
    assert_eq!(network_rows[0].name, Some("worker"));
    assert_eq!(network_rows[0].pid, 42);
    assert_eq!(network_rows[0].first_value, "3.00 KiB/s");
    assert_eq!(network_rows[0].second_value, "1.00 KiB/s");
    assert_eq!(network_rows[0].compact_first_value.as_str(), "3.0K/s");
    assert_eq!(network_rows[0].compact_second_value.as_str(), "1.0K/s");
}

#[test]
fn nvidia_temperature_names_drop_only_known_redundant_prefixes() {
    assert_eq!(
        short_temperature_name("NVIDIA GeForce RTX 2080 Ti"),
        "RTX 2080 Ti"
    );
    assert_eq!(
        short_temperature_name("NVIDIA GeForce GTX 1080"),
        "GTX 1080"
    );
    assert_eq!(short_temperature_name("NVIDIA RTX A4000"), "RTX A4000");
    assert_eq!(
        short_temperature_name("NVIDIA A100-PCIE-40GB"),
        "A100-PCIE-40GB"
    );
    assert_eq!(short_temperature_name("amdgpu"), "amdgpu");
    assert_eq!(short_temperature_name("CPU"), "CPU");
}
