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
use crate::web_transport::WebApiSlice;
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
  <style>
    html, body { margin: 0; padding: 0; overflow: hidden; background: #000; }
    canvas { display: block; }
  </style>
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
    latest: Arc<WebMonitorState>,
    latest_json: Arc<str>,
    subscribers: Vec<SyncSender<Arc<str>>>,
}

impl StateHub {
    #[cfg(any(feature = "http", test))]
    fn new() -> Self {
        let latest = Arc::new(WebMonitorState::from(&MonitorState::default()));
        let latest_json: Arc<str> = serde_json::to_string(latest.as_ref())
            .expect("default monitor state must serialize")
            .into();
        Self {
            inner: Arc::new(Mutex::new(HubState {
                latest,
                latest_json,
                subscribers: Vec::new(),
            })),
        }
    }

    pub fn publish(&self, state: &MonitorState) {
        let latest = Arc::new(WebMonitorState::from(state));
        let Ok(serialized) = serde_json::to_string(latest.as_ref()) else {
            eprintln!("cclover-mon: failed to serialize monitor state for HTTP clients");
            return;
        };
        let serialized: Arc<str> = serialized.into();
        let mut inner = self.inner.lock().expect("HTTP state hub lock poisoned");
        inner.latest = latest;
        inner.latest_json = Arc::clone(&serialized);
        inner.subscribers.retain(
            |subscriber| match subscriber.try_send(Arc::clone(&serialized)) {
                Ok(()) | Err(TrySendError::Full(_)) => true,
                Err(TrySendError::Disconnected(_)) => false,
            },
        );
    }

    #[cfg(feature = "http")]
    fn latest(&self) -> Arc<WebMonitorState> {
        let inner = self.inner.lock().expect("HTTP state hub lock poisoned");
        Arc::clone(&inner.latest)
    }

    #[cfg(feature = "http")]
    fn latest_json(&self) -> Arc<str> {
        let inner = self.inner.lock().expect("HTTP state hub lock poisoned");
        Arc::clone(&inner.latest_json)
    }

    #[cfg(any(feature = "http", test))]
    fn subscribe(&self) -> (Arc<str>, Receiver<Arc<str>>) {
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut inner = self.inner.lock().expect("HTTP state hub lock poisoned");
        let latest = Arc::clone(&inner.latest_json);
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
        "/api/v1/state" => write_json_response(&mut stream, hub.latest_json().as_bytes()),
        "/api/v1/cpu" => write_api_slice(&mut stream, &hub, WebApiSlice::Cpu),
        "/api/v1/memory" => write_api_slice(&mut stream, &hub, WebApiSlice::Memory),
        "/api/v1/disks" => write_api_slice(&mut stream, &hub, WebApiSlice::Disks),
        "/api/v1/networks" => write_api_slice(&mut stream, &hub, WebApiSlice::Networks),
        "/api/v1/temperatures" => write_api_slice(&mut stream, &hub, WebApiSlice::Temperatures),
        "/api/v1/gpus" | "/api/v1/gpu-memory" => {
            write_api_slice(&mut stream, &hub, WebApiSlice::Gpus)
        }
        "/api/v1/processes" => write_api_slice(&mut stream, &hub, WebApiSlice::Processes),
        "/api/v1/history/cpu" => write_api_slice(&mut stream, &hub, WebApiSlice::HistoryCpu),
        "/api/v1/history/memory" => write_api_slice(&mut stream, &hub, WebApiSlice::HistoryMemory),
        "/api/v1/history/gpus" | "/api/v1/history/gpu-memory" => {
            write_api_slice(&mut stream, &hub, WebApiSlice::HistoryGpus)
        }
        "/api/v1/history/disks" => write_api_slice(&mut stream, &hub, WebApiSlice::HistoryDisks),
        "/api/v1/history/networks" => {
            write_api_slice(&mut stream, &hub, WebApiSlice::HistoryNetworks)
        }
        "/api/v1/history/temperatures" => {
            write_api_slice(&mut stream, &hub, WebApiSlice::HistoryTemperatures)
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
fn write_api_slice(stream: &mut TcpStream, hub: &StateHub, slice: WebApiSlice) -> io::Result<()> {
    match hub.latest().serialize_api_slice(slice) {
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
    fn state_api_serves_latest_web_transport_state() {
        let server = HttpServer::start(HttpConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
        })
        .unwrap();

        let state = MonitorState {
            history_capacity: 42,
            ..MonitorState::default()
        };
        server.state_hub().publish(&state);

        let response = get(server.local_addr(), "/api/v1/state");
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(response.contains("Content-Type: application/json\r\n"));
        let (_, body) = response.split_once("\r\n\r\n").unwrap();
        let state: WebMonitorState = serde_json::from_str(body).unwrap();
        let state = MonitorState::from(state);
        assert_eq!(state.history_capacity, 42);
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
