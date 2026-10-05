use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use cclover_core::model::{Collection, GpuId, GpuSnapshot, MonitorState};
use cclover_runtime::{Shutdown, StateSource};

use crate::config::HttpConfig;
use crate::request::parse_request_line;
use crate::server::{HttpServer, wake_address};
use crate::state::StateHub;

#[test]
fn request_line_parsing_is_strict() {
    assert_eq!(
        parse_request_line(b"GET /events HTTP/1.1\r\nHost: localhost\r\n\r\n"),
        Some(("GET", "/events"))
    );
    assert_eq!(parse_request_line(b"broken\r\n\r\n"), None);
}

#[test]
fn unspecified_bind_uses_loopback_to_wake_listener() {
    let address: SocketAddr = "0.0.0.0:9847".parse().unwrap();
    assert_eq!(wake_address(address), "127.0.0.1:9847".parse().unwrap());
}

#[test]
fn state_hub_is_empty_until_first_real_state_then_publishes() {
    let hub = StateHub::new();
    let (initial, receiver) = hub.subscribe();
    assert!(initial.is_none());
    assert!(!hub.is_ready());

    let mut updated = MonitorState::default();
    updated.snapshot.cpu_percent = Collection::Available(42.0);
    hub.publish(updated);
    let received = receiver.recv_timeout(Duration::from_millis(50)).unwrap();
    let received_html: String = serde_json::from_str(&received).unwrap();
    assert!(received_html.contains("cclover-panel"));
    assert!(hub.is_ready());
}

#[test]
fn state_hub_defers_dashboard_render_until_first_subscriber() {
    let hub = StateHub::new();
    let mut state = MonitorState::default();
    state.snapshot.cpu_percent = Collection::Available(42.0);

    hub.publish(state);

    assert!(hub.is_ready());
    assert_eq!(hub.dashboard_render_count(), 0);

    let (initial, receiver) = hub.subscribe();
    let initial = initial.expect("first subscriber must receive the latest real state");
    let initial_html: String = serde_json::from_str(&initial).unwrap();
    assert!(initial_html.contains("cclover-panel"));
    assert_eq!(hub.dashboard_render_count(), 1);

    hub.publish(MonitorState::default());
    assert_eq!(hub.dashboard_render_count(), 2);
    receiver.recv_timeout(Duration::from_millis(50)).unwrap();

    drop(receiver);
    hub.publish(MonitorState::default());
    let renders_after_disconnect_is_observed = hub.dashboard_render_count();
    hub.publish(MonitorState::default());
    assert_eq!(
        hub.dashboard_render_count(),
        renders_after_disconnect_is_observed,
        "dashboard rendering must stop after the disconnected subscriber is retired"
    );
}

#[test]
fn embedded_http_server_serves_page_and_static_assets() {
    let server = HttpServer::start(
        HttpConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
        },
        StateSource::new(),
        Shutdown::default(),
    )
    .unwrap();

    let page = get(server.local_addr(), "/");
    assert!(page.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(page.contains("/bootstrap.js"));
    assert!(page.contains("/style.css"));

    let js = get(server.local_addr(), "/bootstrap.js");
    assert!(js.contains("new EventSource('/events')"));
    assert!(js.contains("getComputedTextLength()"));
    assert!(js.contains("root.innerHTML = previous"));
    assert!(!js.contains("fontSize"));
    let css = get(server.local_addr(), "/style.css");
    assert!(css.contains("Content-Type: text/css; charset=utf-8"));
    assert!(css.contains(".cclover-panel"));
}

#[test]
fn health_is_live_before_state_and_readiness_waits_for_first_state() {
    let server = HttpServer::start(
        HttpConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
        },
        StateSource::new(),
        Shutdown::default(),
    )
    .unwrap();

    assert!(get(server.local_addr(), "/healthz").starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(
        get(server.local_addr(), "/readyz").starts_with("HTTP/1.1 503 Service Unavailable\r\n")
    );
    assert!(
        get(server.local_addr(), "/api/v1/state")
            .starts_with("HTTP/1.1 503 Service Unavailable\r\n")
    );

    server.state_hub().publish(MonitorState::default());
    assert!(get(server.local_addr(), "/readyz").starts_with("HTTP/1.1 200 OK\r\n"));
}

#[test]
fn state_api_serves_latest_api_v1_projection() {
    let server = HttpServer::start(
        HttpConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
        },
        StateSource::new(),
        Shutdown::default(),
    )
    .unwrap();

    let state = MonitorState {
        history_capacity: 42,
        ..MonitorState::default()
    };
    server.state_hub().publish(state);

    let state = response_json(server.local_addr(), "/api/v1/state");
    assert_eq!(state["history_capacity"], 42);
    assert!(state.get("snapshot").is_some());
    assert!(state.get("history").is_some());
}

#[test]
fn split_state_apis_serve_only_requested_domains() {
    let server = HttpServer::start(
        HttpConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
        },
        StateSource::new(),
        Shutdown::default(),
    )
    .unwrap();
    server.state_hub().publish(MonitorState {
        history_capacity: 42,
        ..MonitorState::default()
    });

    for path in [
        "/api/v1/cpu",
        "/api/v1/memory",
        "/api/v1/disks",
        "/api/v1/networks",
        "/api/v1/temperatures",
        "/api/v1/fans",
        "/api/v1/gpus",
        "/api/v1/gpu-memory",
        "/api/v1/processes",
        "/api/v1/history/cpu",
        "/api/v1/history/memory",
        "/api/v1/history/gpus",
        "/api/v1/history/gpu-memory",
        "/api/v1/history/disks",
        "/api/v1/history/networks",
        "/api/v1/history/temperatures",
        "/api/v1/history/fans",
    ] {
        let _ = response_json(server.local_addr(), path);
    }

    let cpu = response_json(server.local_addr(), "/api/v1/cpu");
    assert!(cpu.get("cpu_percent").is_some());
    assert!(cpu.get("top_cpu").is_some());
    assert!(cpu.get("memory").is_none());
    assert!(cpu.get("history").is_none());

    let processes = response_json(server.local_addr(), "/api/v1/processes");
    assert!(processes.get("top_cpu").is_some());
    assert!(processes.get("top_memory").is_some());
    assert!(processes.get("disk_io").is_some());
    assert!(processes.get("network_io").is_some());

    let history = response_json(server.local_addr(), "/api/v1/history/cpu");
    assert_eq!(history["history_capacity"], 42);
    assert!(history.get("cpu").is_some());
    assert!(history.get("memory_used").is_none());
}

#[test]
fn legacy_gpu_memory_routes_preserve_payload_not_only_url() {
    let server = HttpServer::start(
        HttpConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
        },
        StateSource::new(),
        Shutdown::default(),
    )
    .unwrap();
    let gpu_id = GpuId::from_opaque_key("gpu-a");
    let mut state = MonitorState {
        history_capacity: 120,
        ..MonitorState::default()
    };
    state.snapshot.gpus = Collection::available(vec![GpuSnapshot {
        id: gpu_id.clone(),
        name: "GPU A".to_owned(),
        utilization_percent: Some(42.0),
        memory_used_bytes: Some(4),
        memory_total_bytes: Some(8),
        temperature_celsius: Some(63.0),
        power_watts: None,
        core_clock_mhz: None,
        fan_percent: None,
        fan_rpm: None,
    }]);
    state
        .history
        .gpu_memory_used
        .insert(gpu_id, std::collections::VecDeque::from([2.0, 4.0]));
    server.state_hub().publish(state);

    let gpus = response_json(server.local_addr(), "/api/v1/gpus");
    let legacy = response_json(server.local_addr(), "/api/v1/gpu-memory");
    let legacy_history = response_json(server.local_addr(), "/api/v1/history/gpu-memory");

    assert!(gpus.get("gpus").is_some());
    assert!(gpus.get("gpu_memory").is_none());
    assert_eq!(
        legacy,
        serde_json::json!({
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
        legacy_history,
        serde_json::json!({
            "history_capacity": 120,
            "gpu_memory_used": {"gpu-a": [2.0, 4.0]}
        })
    );
}

fn get(address: SocketAddr, path: &str) -> String {
    String::from_utf8(get_bytes(address, path)).unwrap()
}

fn response_json(address: SocketAddr, path: &str) -> serde_json::Value {
    let response = get(address, path);
    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(response.contains("Content-Type: application/json\r\n"));
    let (_, body) = response.split_once("\r\n\r\n").unwrap();
    serde_json::from_str(body).unwrap()
}

fn get_bytes(address: SocketAddr, path: &str) -> Vec<u8> {
    let mut stream = TcpStream::connect(address).unwrap();
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    response
}
