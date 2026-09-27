use std::io::{self, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::core::model::MonitorState;
use crate::web_transport::WebMonitorState;

pub const DEFAULT_HTTP_BIND: SocketAddr =
    SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), 9847);

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

const BOOTSTRAP_JS: &str = r#"import init from './cclover_mon_web.js';
await init({ module_or_path: './cclover_mon_web_bg.wasm' });
"#;
const WEB_JS: &str = include_str!(concat!(env!("OUT_DIR"), "/web/cclover_mon_web.js"));
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
    latest: Arc<str>,
    subscribers: Vec<SyncSender<Arc<str>>>,
}

impl StateHub {
    fn new() -> Self {
        let latest: Arc<str> =
            serde_json::to_string(&WebMonitorState::from(&MonitorState::default()))
                .expect("default monitor state must serialize")
                .into();
        Self {
            inner: Arc::new(Mutex::new(HubState {
                latest,
                subscribers: Vec::new(),
            })),
        }
    }

    pub fn publish(&self, state: &MonitorState) {
        let Ok(serialized) = serde_json::to_string(&WebMonitorState::from(state)) else {
            eprintln!("cclover-mon: failed to serialize monitor state for HTTP clients");
            return;
        };
        let serialized: Arc<str> = serialized.into();
        let mut inner = self.inner.lock().expect("HTTP state hub lock poisoned");
        inner.latest = Arc::clone(&serialized);
        inner.subscribers.retain(
            |subscriber| match subscriber.try_send(Arc::clone(&serialized)) {
                Ok(()) | Err(TrySendError::Full(_)) => true,
                Err(TrySendError::Disconnected(_)) => false,
            },
        );
    }

    fn subscribe(&self) -> (Arc<str>, Receiver<Arc<str>>) {
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut inner = self.inner.lock().expect("HTTP state hub lock poisoned");
        let latest = Arc::clone(&inner.latest);
        inner.subscribers.push(sender);
        (latest, receiver)
    }
}

pub struct HttpServer {
    hub: StateHub,
    local_addr: SocketAddr,
    wake_addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

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

impl Drop for HttpServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        let _ = TcpStream::connect_timeout(&self.wake_addr, Duration::from_millis(250));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

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
        "/events" => stream_events(stream, hub, shutdown),
        _ => write_response(
            &mut stream,
            404,
            "text/plain; charset=utf-8",
            b"not found\n",
        ),
    }
}

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

fn write_event(stream: &mut TcpStream, state: &str) -> io::Result<()> {
    stream.write_all(b"data: ")?;
    stream.write_all(state.as_bytes())?;
    stream.write_all(b"\n\n")?;
    stream.flush()
}

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

    fn get(address: SocketAddr, path: &str) -> String {
        String::from_utf8(get_bytes(address, path)).unwrap()
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
}
