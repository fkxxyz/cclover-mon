use std::io;
#[cfg(feature = "http")]
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr};
#[cfg(feature = "http")]
use std::net::{TcpListener, TcpStream};
#[cfg(feature = "http")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "http")]
use std::sync::mpsc::RecvTimeoutError;
#[cfg(any(feature = "http", test))]
use std::sync::mpsc::{self, Receiver};
use std::sync::mpsc::{SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
#[cfg(feature = "http")]
use std::thread::{self, JoinHandle};
#[cfg(any(feature = "http", test))]
use std::time::Duration;

use crate::core::model::MonitorState;
#[cfg(feature = "http")]
use crate::web_api::{ApiV1Slice, ApiV1State};
use crate::web_transport::WebMonitorState;

pub const DEFAULT_HTTP_BIND: SocketAddr =
    SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), 9847);

#[cfg(feature = "http")]
const INDEX_HTML: &str = r#"<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>cclover-mon</title>
</head>
<body>
  <script type="module" src="/bootstrap.js"></script>
</body>
</html>
"#;

#[cfg(feature = "http")]
const BOOTSTRAP_JS: &str = r#"import init from './cclover_mon_web.js';
await init({ module_or_path: './cclover_mon_web_bg.wasm' });
"#;
#[cfg(feature = "http")]
const WEB_JS: &str = include_str!(concat!(env!("OUT_DIR"), "/web/cclover_mon_web.js"));
#[cfg(feature = "http")]
const WEB_WASM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/web/cclover_mon_web_bg.wasm"));

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HttpConfig {
    pub bind: SocketAddr,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            bind: DEFAULT_HTTP_BIND,
        }
    }
}

#[derive(Clone)]
pub struct StateHub {
    inner: Arc<Mutex<HubState>>,
}

struct HubState {
    latest_transport_json: Arc<str>,
    #[cfg(feature = "http")]
    latest_api_v1: Arc<ApiV1State>,
    #[cfg(feature = "http")]
    latest_api_v1_json: Arc<str>,
    subscribers: Vec<SyncSender<Arc<str>>>,
}

impl StateHub {
    #[cfg(any(feature = "http", test))]
    fn new() -> Self {
        let default_state = MonitorState::default();
        let latest_transport = Arc::new(WebMonitorState::from(&default_state));
        let latest_transport_json: Arc<str> = serde_json::to_string(latest_transport.as_ref())
            .expect("default browser transport state must serialize")
            .into();
        #[cfg(feature = "http")]
        let latest_api_v1 = Arc::new(ApiV1State::from(&default_state));
        #[cfg(feature = "http")]
        let latest_api_v1_json: Arc<str> = serde_json::to_string(latest_api_v1.as_ref())
            .expect("default API v1 state must serialize")
            .into();
        Self {
            inner: Arc::new(Mutex::new(HubState {
                latest_transport_json,
                #[cfg(feature = "http")]
                latest_api_v1,
                #[cfg(feature = "http")]
                latest_api_v1_json,
                subscribers: Vec::new(),
            })),
        }
    }

    pub fn publish(&self, state: &MonitorState) {
        let latest_transport = Arc::new(WebMonitorState::from(state));
        let Ok(transport_json) = serde_json::to_string(latest_transport.as_ref()) else {
            eprintln!("cclover-mon: failed to serialize browser transport state");
            return;
        };
        #[cfg(feature = "http")]
        let latest_api_v1 = Arc::new(ApiV1State::from(state));
        #[cfg(feature = "http")]
        let Ok(api_v1_json) = serde_json::to_string(latest_api_v1.as_ref()) else {
            eprintln!("cclover-mon: failed to serialize API v1 state");
            return;
        };
        let transport_json: Arc<str> = transport_json.into();
        #[cfg(feature = "http")]
        let api_v1_json: Arc<str> = api_v1_json.into();
        let mut inner = self.inner.lock().expect("HTTP state hub lock poisoned");
        inner.latest_transport_json = Arc::clone(&transport_json);
        #[cfg(feature = "http")]
        {
            inner.latest_api_v1 = latest_api_v1;
            inner.latest_api_v1_json = api_v1_json;
        }
        inner.subscribers.retain(|subscriber| {
            match subscriber.try_send(Arc::clone(&transport_json)) {
                Ok(()) | Err(TrySendError::Full(_)) => true,
                Err(TrySendError::Disconnected(_)) => false,
            }
        });
    }

    #[cfg(feature = "http")]
    fn latest_api_v1(&self) -> Arc<ApiV1State> {
        let inner = self.inner.lock().expect("HTTP state hub lock poisoned");
        Arc::clone(&inner.latest_api_v1)
    }

    #[cfg(feature = "http")]
    fn latest_api_v1_json(&self) -> Arc<str> {
        let inner = self.inner.lock().expect("HTTP state hub lock poisoned");
        Arc::clone(&inner.latest_api_v1_json)
    }

    #[cfg(any(feature = "http", test))]
    fn subscribe(&self) -> (Arc<str>, Receiver<Arc<str>>) {
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut inner = self.inner.lock().expect("HTTP state hub lock poisoned");
        let latest = Arc::clone(&inner.latest_transport_json);
        inner.subscribers.push(sender);
        (latest, receiver)
    }
}

#[cfg(not(feature = "http"))]
pub struct HttpServer;

#[cfg(not(feature = "http"))]
impl HttpServer {
    pub fn start(_config: HttpConfig) -> io::Result<Self> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "HTTP monitoring is unavailable in this build; rebuild with the `http` feature",
        ))
    }

    pub fn state_hub(&self) -> StateHub {
        unreachable!("HTTP server cannot exist without the `http` feature")
    }
}

#[cfg(feature = "http")]
pub struct HttpServer {
    hub: StateHub,
    local_addr: SocketAddr,
    wake_addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

#[cfg(feature = "http")]
impl HttpServer {
    pub fn start(config: HttpConfig) -> io::Result<Self> {
        let listener = TcpListener::bind(config.bind)?;
        let local_addr = listener.local_addr()?;
        let wake_addr = wake_address(local_addr);
        let hub = StateHub::new();
        let server_hub = hub.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        let server_shutdown = Arc::clone(&shutdown);

        let thread = thread::Builder::new()
            .name("cclover-mon-http".to_owned())
            .spawn(move || run_listener(listener, server_hub, server_shutdown))?;

        eprintln!("cclover-mon: HTTP monitor listening on http://{local_addr}");
        Ok(Self {
            hub,
            local_addr,
            wake_addr,
            shutdown,
            thread: Some(thread),
        })
    }

    pub fn state_hub(&self) -> StateHub {
        self.hub.clone()
    }

    pub const fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
}

#[cfg(feature = "http")]
impl Drop for HttpServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        let _ = TcpStream::connect_timeout(&self.wake_addr, Duration::from_millis(250));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(feature = "http")]
fn run_listener(listener: TcpListener, hub: StateHub, shutdown: Arc<AtomicBool>) {
    while !shutdown.load(Ordering::Acquire) {
        let (stream, _) = match listener.accept() {
            Ok(connection) => connection,
            Err(error) => {
                if !shutdown.load(Ordering::Acquire) {
                    eprintln!("cclover-mon: HTTP accept failed: {error}");
                }
                continue;
            }
        };
        if shutdown.load(Ordering::Acquire) {
            break;
        }

        let client_hub = hub.clone();
        let client_shutdown = Arc::clone(&shutdown);
        if let Err(error) = thread::Builder::new()
            .name("cclover-mon-http-client".to_owned())
            .spawn(move || {
                if let Err(error) = handle_connection(stream, client_hub, client_shutdown)
                    && error.kind() != io::ErrorKind::BrokenPipe
                    && error.kind() != io::ErrorKind::ConnectionReset
                {
                    eprintln!("cclover-mon: HTTP client failed: {error}");
                }
            })
        {
            eprintln!("cclover-mon: failed to spawn HTTP client handler: {error}");
        }
    }
}

#[cfg(feature = "http")]
fn handle_connection(
    mut stream: TcpStream,
    hub: StateHub,
    shutdown: Arc<AtomicBool>,
) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let request = read_request(&mut stream)?;
    let Some((method, path)) = parse_request_line(&request) else {
        return write_response(
            &mut stream,
            400,
            "text/plain; charset=utf-8",
            b"bad request\n",
        );
    };
    if method != "GET" {
        return write_response(
            &mut stream,
            405,
            "text/plain; charset=utf-8",
            b"method not allowed\n",
        );
    }

    match path.split('?').next().unwrap_or(path) {
        "/" => write_response(
            &mut stream,
            200,
            "text/html; charset=utf-8",
            INDEX_HTML.as_bytes(),
        ),
        "/bootstrap.js" => write_response(
            &mut stream,
            200,
            "text/javascript; charset=utf-8",
            BOOTSTRAP_JS.as_bytes(),
        ),
        "/cclover_mon_web.js" => write_response(
            &mut stream,
            200,
            "text/javascript; charset=utf-8",
            WEB_JS.as_bytes(),
        ),
        "/cclover_mon_web_bg.wasm" => {
            write_response(&mut stream, 200, "application/wasm", WEB_WASM)
        }
        "/api/v1/state" => write_json_response(&mut stream, hub.latest_api_v1_json().as_bytes()),
        "/api/v1/cpu" => write_api_slice(&mut stream, &hub, ApiV1Slice::Cpu),
        "/api/v1/memory" => write_api_slice(&mut stream, &hub, ApiV1Slice::Memory),
        "/api/v1/disks" => write_api_slice(&mut stream, &hub, ApiV1Slice::Disks),
        "/api/v1/networks" => write_api_slice(&mut stream, &hub, ApiV1Slice::Networks),
        "/api/v1/temperatures" => write_api_slice(&mut stream, &hub, ApiV1Slice::Temperatures),
        "/api/v1/gpus" => write_api_slice(&mut stream, &hub, ApiV1Slice::Gpus),
        "/api/v1/gpu-memory" => write_api_slice(&mut stream, &hub, ApiV1Slice::GpuMemoryLegacy),
        "/api/v1/processes" => write_api_slice(&mut stream, &hub, ApiV1Slice::Processes),
        "/api/v1/history/cpu" => write_api_slice(&mut stream, &hub, ApiV1Slice::HistoryCpu),
        "/api/v1/history/memory" => write_api_slice(&mut stream, &hub, ApiV1Slice::HistoryMemory),
        "/api/v1/history/gpus" => write_api_slice(&mut stream, &hub, ApiV1Slice::HistoryGpus),
        "/api/v1/history/gpu-memory" => {
            write_api_slice(&mut stream, &hub, ApiV1Slice::HistoryGpuMemoryLegacy)
        }
        "/api/v1/history/disks" => write_api_slice(&mut stream, &hub, ApiV1Slice::HistoryDisks),
        "/api/v1/history/networks" => {
            write_api_slice(&mut stream, &hub, ApiV1Slice::HistoryNetworks)
        }
        "/api/v1/history/temperatures" => {
            write_api_slice(&mut stream, &hub, ApiV1Slice::HistoryTemperatures)
        }
        "/events" => stream_events(stream, hub, shutdown),
        _ => write_response(
            &mut stream,
            404,
            "text/plain; charset=utf-8",
            b"not found\n",
        ),
    }
}

#[cfg(feature = "http")]
fn write_api_slice(stream: &mut TcpStream, hub: &StateHub, slice: ApiV1Slice) -> io::Result<()> {
    match hub.latest_api_v1().serialize_slice(slice) {
        Ok(body) => write_json_response(stream, body.as_bytes()),
        Err(error) => {
            eprintln!("cclover-mon: failed to serialize HTTP API response: {error}");
            write_response(
                stream,
                500,
                "text/plain; charset=utf-8",
                b"internal server error\n",
            )
        }
    }
}

#[cfg(feature = "http")]
fn write_json_response(stream: &mut TcpStream, body: &[u8]) -> io::Result<()> {
    write_response(stream, 200, "application/json", body)
}

#[cfg(feature = "http")]
fn read_request(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    const LIMIT: usize = 16 * 1024;
    let mut request = Vec::with_capacity(1024);
    let mut buffer = [0_u8; 1024];
    while request.len() < LIMIT {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            return Ok(request);
        }
    }
    if request.len() >= LIMIT {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "HTTP request headers too large",
        ));
    }
    Ok(request)
}

#[cfg(feature = "http")]
fn parse_request_line(request: &[u8]) -> Option<(&str, &str)> {
    let request = std::str::from_utf8(request).ok()?;
    let line = request.lines().next()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?;
    let path = parts.next()?;
    let version = parts.next()?;
    if !version.starts_with("HTTP/1.") || parts.next().is_some() {
        return None;
    }
    Some((method, path))
}

#[cfg(feature = "http")]
fn write_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        _ => "Error",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self'; style-src 'unsafe-inline'; object-src 'none'\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    stream.flush()
}

#[cfg(feature = "http")]
fn stream_events(
    mut stream: TcpStream,
    hub: StateHub,
    shutdown: Arc<AtomicBool>,
) -> io::Result<()> {
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nX-Accel-Buffering: no\r\nX-Content-Type-Options: nosniff\r\nConnection: keep-alive\r\n\r\n"
    )?;

    let (latest, receiver) = hub.subscribe();
    write_event(&mut stream, &latest)?;
    while !shutdown.load(Ordering::Acquire) {
        match receiver.recv_timeout(Duration::from_secs(15)) {
            Ok(state) => write_event(&mut stream, &state)?,
            Err(RecvTimeoutError::Timeout) => {
                stream.write_all(b": keepalive\n\n")?;
                stream.flush()?;
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    Ok(())
}

#[cfg(feature = "http")]
fn write_event(stream: &mut TcpStream, state: &str) -> io::Result<()> {
    stream.write_all(b"data: ")?;
    stream.write_all(state.as_bytes())?;
    stream.write_all(b"\n\n")?;
    stream.flush()
}

#[cfg(feature = "http")]
fn wake_address(address: SocketAddr) -> SocketAddr {
    match address.ip() {
        IpAddr::V4(ip) if ip.is_unspecified() => {
            SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), address.port())
        }
        IpAddr::V6(ip) if ip.is_unspecified() => {
            SocketAddr::new(IpAddr::V6(std::net::Ipv6Addr::LOCALHOST), address.port())
        }
        _ => address,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "http")]
    use crate::core::model::{Collection, GpuId, GpuSnapshot};

    #[cfg(feature = "http")]
    #[test]
    fn request_line_parsing_is_strict() {
        assert_eq!(
            parse_request_line(b"GET /events HTTP/1.1\r\nHost: localhost\r\n\r\n"),
            Some(("GET", "/events"))
        );
        assert_eq!(parse_request_line(b"broken\r\n\r\n"), None);
    }

    #[cfg(feature = "http")]
    #[test]
    fn unspecified_bind_uses_loopback_to_wake_listener() {
        let address: SocketAddr = "0.0.0.0:9847".parse().unwrap();
        assert_eq!(wake_address(address), "127.0.0.1:9847".parse().unwrap());
    }

    #[test]
    fn state_hub_sends_latest_then_updates() {
        let hub = StateHub::new();
        let (initial, receiver) = hub.subscribe();
        let initial_state: WebMonitorState = serde_json::from_str(&initial).unwrap();
        let initial_state = MonitorState::from(initial_state);
        assert_eq!(initial_state.history_capacity, 0);

        let updated = MonitorState {
            history_capacity: 42,
            ..MonitorState::default()
        };
        hub.publish(&updated);
        let received = receiver.recv_timeout(Duration::from_millis(50)).unwrap();
        let received: WebMonitorState = serde_json::from_str(&received).unwrap();
        let received = MonitorState::from(received);
        assert_eq!(received.history_capacity, 42);
    }

    #[cfg(feature = "http")]
    #[test]
    fn embedded_http_server_serves_page_and_wasm() {
        let server = HttpServer::start(HttpConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
        })
        .unwrap();

        let page = get(server.local_addr(), "/");
        assert!(page.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(page.contains("/bootstrap.js"));

        let wasm = get_bytes(server.local_addr(), "/cclover_mon_web_bg.wasm");
        assert!(wasm.starts_with(b"HTTP/1.1 200 OK\r\n"));
        let wasm_content_type = b"Content-Type: application/wasm";
        assert!(
            wasm.windows(wasm_content_type.len())
                .any(|bytes| bytes == wasm_content_type)
        );
    }

    #[cfg(feature = "http")]
    #[test]
    fn state_api_serves_latest_api_v1_projection() {
        let server = HttpServer::start(HttpConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
        })
        .unwrap();

        let state = MonitorState {
            history_capacity: 42,
            ..MonitorState::default()
        };
        server.state_hub().publish(&state);

        let state = response_json(server.local_addr(), "/api/v1/state");
        assert_eq!(state["history_capacity"], 42);
        assert!(state.get("snapshot").is_some());
        assert!(state.get("history").is_some());
    }

    #[cfg(feature = "http")]
    #[test]
    fn split_state_apis_serve_only_requested_domains() {
        let server = HttpServer::start(HttpConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
        })
        .unwrap();
        server.state_hub().publish(&MonitorState {
            history_capacity: 42,
            ..MonitorState::default()
        });

        for path in [
            "/api/v1/cpu",
            "/api/v1/memory",
            "/api/v1/disks",
            "/api/v1/networks",
            "/api/v1/temperatures",
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

    #[cfg(feature = "http")]
    #[test]
    fn legacy_gpu_memory_routes_preserve_payload_not_only_url() {
        let server = HttpServer::start(HttpConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
        })
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
        server.state_hub().publish(&state);

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

    #[cfg(feature = "http")]
    fn get(address: SocketAddr, path: &str) -> String {
        String::from_utf8(get_bytes(address, path)).unwrap()
    }

    #[cfg(feature = "http")]
    fn response_json(address: SocketAddr, path: &str) -> serde_json::Value {
        let response = get(address, path);
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(response.contains("Content-Type: application/json\r\n"));
        let (_, body) = response.split_once("\r\n\r\n").unwrap();
        serde_json::from_str(body).unwrap()
    }

    #[cfg(feature = "http")]
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
}
