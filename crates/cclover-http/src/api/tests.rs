use std::collections::VecDeque;

use cclover_core::model::{
    Collection, CollectionStatus, CollectionUnavailable, DiskId, GpuId, GpuSnapshot,
    MemorySnapshot, MonitorState, ProcessDiskIoSnapshot, ProcessInstanceId,
    ProcessNetworkIoSnapshot, ProcessSnapshot,
};
use serde_json::json;

use super::model::*;

fn representative_state() -> MonitorState {
    let mut state = MonitorState {
        history_capacity: 120,
        ..MonitorState::default()
    };
    state.snapshot.cpu_percent = Collection::available(37.5);
    state.snapshot.memory = Collection::available(MemorySnapshot {
        used_bytes: 10,
        total_bytes: 20,
        swap_used_bytes: 3,
        swap_total_bytes: 4,
    });
    state.snapshot.gpus = Collection::available(vec![
        GpuSnapshot {
            id: GpuId::from_opaque_key("gpu-a"),
            name: "GPU A".to_owned(),
            utilization_percent: Some(42.0),
            memory_used_bytes: Some(4),
            memory_total_bytes: Some(8),
            temperature_celsius: Some(63.0),
            power_watts: Some(145.0),
            core_clock_mhz: Some(1830),
            fan_percent: Some(37.0),
            fan_rpm: None,
        },
        GpuSnapshot {
            id: GpuId::from_opaque_key("gpu-without-memory"),
            name: "GPU B".to_owned(),
            utilization_percent: Some(5.0),
            memory_used_bytes: None,
            memory_total_bytes: None,
            temperature_celsius: None,
            power_watts: None,
            core_clock_mhz: None,
            fan_percent: None,
            fan_rpm: None,
        },
    ]);
    let worker = ProcessInstanceId {
        pid: 42,
        instance: 7,
    };
    let unattributed = ProcessInstanceId {
        pid: 43,
        instance: 9007199254740993,
    };
    state.snapshot.processes = cclover_core::model::ProcessDomainSnapshot {
        by_id: std::sync::Arc::new(
            [
                (
                    worker,
                    ProcessSnapshot {
                        id: worker,
                        name: Some("worker".into()),
                        cpu_percent: Some(12.5),
                        memory_bytes: Some(4096),
                        disk_io: vec![ProcessDiskIoSnapshot {
                            disk_id: DiskId::from_opaque_key("disk-a"),
                            device: "nvme0n1".to_owned(),
                            read_bytes_per_sec: 1.0,
                            write_bytes_per_sec: 2.0,
                        }],
                        network_io: vec![ProcessNetworkIoSnapshot {
                            network_id: cclover_core::model::NetworkId::from_opaque_key(
                                "network-a",
                            ),
                            interface: "eth0".to_owned(),
                            rx_bytes_per_sec: 8.0,
                            tx_bytes_per_sec: 16.0,
                        }],
                    },
                ),
                (
                    unattributed,
                    ProcessSnapshot {
                        id: unattributed,
                        name: None,
                        cpu_percent: Some(0.5),
                        memory_bytes: Some(512),
                        disk_io: Vec::new(),
                        network_io: Vec::new(),
                    },
                ),
            ]
            .into_iter()
            .collect(),
        ),
        metadata_status: CollectionStatus::Available,
        cpu_status: CollectionStatus::Available,
        memory_status: CollectionStatus::Available,
        disk_io_status: CollectionStatus::Available,
        network_io_status: CollectionStatus::Available,
    };
    state.history.cpu = VecDeque::from([11.0, 12.0]);
    state
        .history
        .gpu_memory_used
        .insert(GpuId::from_opaque_key("gpu-a"), VecDeque::from([2.0, 4.0]));
    state
}

#[test]
fn gpu_wire_shape_is_an_explicit_v1_contract() {
    let state = ApiV1State::from(&representative_state());
    let actual: serde_json::Value =
        serde_json::from_str(&state.serialize_slice(ApiV1Slice::Gpus).unwrap()).unwrap();

    assert_eq!(
        actual,
        json!({
            "gpus": {
                "status": "available",
                "value": [
                    {
                        "id": "gpu-a",
                        "name": "GPU A",
                        "utilization_percent": 42.0,
                        "memory_used_bytes": 4,
                        "memory_total_bytes": 8,
                        "temperature_celsius": 63.0,
                        "power_watts": 145.0,
                        "core_clock_mhz": 1830,
                        "fan_percent": 37.0,
                        "fan_rpm": null
                    },
                    {
                        "id": "gpu-without-memory",
                        "name": "GPU B",
                        "utilization_percent": 5.0,
                        "memory_used_bytes": null,
                        "memory_total_bytes": null,
                        "temperature_celsius": null,
                        "power_watts": null,
                        "core_clock_mhz": null,
                        "fan_percent": null,
                        "fan_rpm": null
                    }
                ]
            }
        })
    );
}

#[test]
fn collection_status_and_unavailable_reason_are_part_of_v1() {
    let mut state = MonitorState::default();
    state.snapshot.cpu_percent = Collection::unavailable(CollectionUnavailable::PermissionDenied);
    state.snapshot.networks = Collection::degraded(Vec::new());
    let api = ApiV1State::from(&state);

    let cpu: serde_json::Value =
        serde_json::from_str(&api.serialize_slice(ApiV1Slice::Cpu).unwrap()).unwrap();
    let networks: serde_json::Value =
        serde_json::from_str(&api.serialize_slice(ApiV1Slice::Networks).unwrap()).unwrap();

    assert_eq!(
        cpu["cpu_percent"],
        json!({"status": "unavailable", "value": "permission_denied"})
    );
    assert_eq!(
        networks["networks"],
        json!({"status": "degraded", "value": []})
    );
}

#[test]
fn process_rows_carry_identity_and_never_drop_a_row_for_a_missing_name() {
    let state = ApiV1State::from(&representative_state());
    let actual: serde_json::Value =
        serde_json::from_str(&state.serialize_slice(ApiV1Slice::Processes).unwrap()).unwrap();

    assert_eq!(
        actual,
        json!({
            "top_cpu": {
                "status": "available",
                "value": [
                    {
                        "process": {"pid": 42, "instance": "7"},
                        "name": "worker",
                        "percent": 12.5
                    },
                    {
                        "process": {"pid": 43, "instance": "9007199254740993"},
                        "name": null,
                        "percent": 0.5
                    }
                ]
            },
            "top_memory": {
                "status": "available",
                "value": [
                    {
                        "process": {"pid": 42, "instance": "7"},
                        "name": "worker",
                        "bytes": 4096
                    },
                    {
                        "process": {"pid": 43, "instance": "9007199254740993"},
                        "name": null,
                        "bytes": 512
                    }
                ]
            },
            "disk_io": {
                "status": "available",
                "value": [{
                    "process": {"pid": 42, "instance": "7"},
                    "name": "worker",
                    "disk_id": "disk-a",
                    "device": "nvme0n1",
                    "read_bytes_per_sec": 1.0,
                    "write_bytes_per_sec": 2.0
                }]
            },
            "network_io": {
                "status": "available",
                "value": [{
                    "process": {"pid": 42, "instance": "7"},
                    "name": "worker",
                    "network_id": "network-a",
                    "interface": "eth0",
                    "rx_bytes_per_sec": 8.0,
                    "tx_bytes_per_sec": 16.0
                }]
            }
        })
    );
}

#[test]
fn full_state_shape_is_owned_by_api_v1() {
    let actual = serde_json::to_value(ApiV1State::from(&representative_state())).unwrap();

    assert_eq!(actual["history_capacity"], 120);
    assert_eq!(actual["snapshot"]["cpu_percent"]["status"], "available");
    assert_eq!(actual["snapshot"]["cpu_percent"]["value"], 37.5);
    assert_eq!(actual["snapshot"]["gpus"]["value"][0]["id"], "gpu-a");
    assert_eq!(
        actual["snapshot"]["top_cpu"]["value"][0]["process"]["instance"],
        json!("7"),
        "the process instance marker is a string, never a JSON number"
    );
    assert_eq!(actual["history"]["cpu"], json!([11.0, 12.0]));
}
