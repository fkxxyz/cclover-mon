use super::*;
use std::collections::VecDeque;

use crate::core::model::{
    Collection, CollectionUnavailable, GpuId, GpuSnapshot, MemorySnapshot, MonitorState,
    ProcessDiskIo, ProcessInstanceId,
};
use serde_json::json;

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
    state.snapshot.process_disk_io = Collection::available(vec![ProcessDiskIo {
        process: ProcessInstanceId {
            pid: 42,
            birth_marker: 7,
        },
        name: Some("worker".into()),
        disk_id: crate::core::model::DiskId::from_opaque_key("disk-a"),
        device: "nvme0n1".to_owned(),
        read_bytes_per_sec: 1.0,
        write_bytes_per_sec: 2.0,
    }]);
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
fn legacy_gpu_memory_routes_keep_their_original_payload_shape() {
    let state = ApiV1State::from(&representative_state());
    let snapshot: serde_json::Value =
        serde_json::from_str(&state.serialize_slice(ApiV1Slice::GpuMemoryLegacy).unwrap()).unwrap();
    let history: serde_json::Value = serde_json::from_str(
        &state
            .serialize_slice(ApiV1Slice::HistoryGpuMemoryLegacy)
            .unwrap(),
    )
    .unwrap();

    assert_eq!(
        snapshot,
        json!({
            "gpu_memory": {
                "status": "available",
                "value": [{
                    "id": "gpu-a",
                    "name": "GPU A",
                    "used_bytes": 4,
                    "total_bytes": 8
                }]
            }
        })
    );
    assert_eq!(
        history,
        json!({
            "history_capacity": 120,
            "gpu_memory_used": {"gpu-a": [2.0, 4.0]}
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
fn full_state_shape_is_owned_by_api_v1() {
    let actual = serde_json::to_value(ApiV1State::from(&representative_state())).unwrap();

    assert_eq!(actual["history_capacity"], 120);
    assert_eq!(actual["snapshot"]["cpu_percent"]["status"], "available");
    assert_eq!(actual["snapshot"]["cpu_percent"]["value"], 37.5);
    assert_eq!(actual["snapshot"]["gpus"]["value"][0]["id"], "gpu-a");
    assert_eq!(actual["history"]["cpu"], json!([11.0, 12.0]));
    assert!(actual["snapshot"].get("gpu_memory").is_none());
}
