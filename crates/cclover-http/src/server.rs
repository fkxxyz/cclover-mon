use std::io;
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use cclover_runtime::{Shutdown, StateSource};
use crossbeam_channel::{RecvError, select};

use crate::config::HttpConfig;
use crate::request::write_response;
use crate::routes::handle_connection;
use crate::state::StateHub;

pub struct HttpServer {
    #[cfg(test)]
    hub: StateHub,
    local_addr: SocketAddr,
    shutdown: Shutdown,
    wake_thread: Option<JoinHandle<()>>,
    state_thread: Option<JoinHandle<()>>,
    listener_thread: Option<JoinHandle<()>>,
}

impl HttpServer {
    pub fn start(config: HttpConfig, states: StateSource, shutdown: Shutdown) -> io::Result<Self> {
        let listener = TcpListener::bind(config.bind)?;
        let local_addr = listener.local_addr()?;
        let wake_addr = wake_address(local_addr);
        let hub = StateHub::new();

        let listener_hub = hub.clone();
        let listener_shutdown = shutdown.clone();
        let listener_thread = thread::Builder::new()
            .name("cclover-mon-http".to_owned())
            .spawn(move || run_listener(listener, listener_hub, listener_shutdown))?;

        let wake_shutdown = shutdown.clone();
        let wake_thread = match thread::Builder::new()
            .name("cclover-mon-http-wake".to_owned())
            .spawn(move || {
                wake_shutdown.wait();
                let _ = TcpStream::connect_timeout(&wake_addr, Duration::from_millis(250));
            }) {
            Ok(thread) => thread,
            Err(error) => {
                shutdown.request();
                let _ = TcpStream::connect_timeout(&wake_addr, Duration::from_millis(250));
                let _ = listener_thread.join();
                return Err(error);
            }
        };

        let state_hub = hub.clone();
        let state_receiver = states.subscribe();
        let state_shutdown = shutdown.subscribe();
        let state_thread = match thread::Builder::new()
            .name("cclover-mon-http-state".to_owned())
            .spawn(move || {
                loop {
                    select! {
                        recv(state_shutdown) -> _ => break,
                        recv(state_receiver) -> state => match state {
                            Ok(state) => state_hub.publish(&state),
                            Err(RecvError) => break,
                        }
                    }
                }
            }) {
            Ok(thread) => thread,
            Err(error) => {
                shutdown.request();
                let _ = wake_thread.join();
                let _ = listener_thread.join();
                return Err(error);
            }
        };

        eprintln!("cclover-mon: HTTP monitor listening on http://{local_addr}");
        Ok(Self {
            #[cfg(test)]
            hub,
            local_addr,
            shutdown,
            wake_thread: Some(wake_thread),
            state_thread: Some(state_thread),
            listener_thread: Some(listener_thread),
        })
    }

    #[cfg(test)]
    pub(crate) fn state_hub(&self) -> StateHub {
        self.hub.clone()
    }

    pub const fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
}

impl Drop for HttpServer {
    fn drop(&mut self) {
        self.shutdown.request();
        if let Some(thread) = self.state_thread.take() {
            let _ = thread.join();
        }
        if let Some(thread) = self.listener_thread.take() {
            let _ = thread.join();
        }
        if let Some(thread) = self.wake_thread.take() {
            let _ = thread.join();
        }
    }
}

const MAX_HTTP_CLIENTS: usize = 64;

fn run_listener(listener: TcpListener, hub: StateHub, shutdown: Shutdown) {
    let active_clients = Arc::new(AtomicUsize::new(0));
    let mut clients = Vec::new();
    while !shutdown.is_requested() {
        let (stream, _) = match listener.accept() {
            Ok(connection) => connection,
            Err(error) => {
                if !shutdown.is_requested() {
                    eprintln!("cclover-mon: HTTP accept failed: {error}");
                }
                continue;
            }
        };
        if shutdown.is_requested() {
            break;
        }

        reap_finished(&mut clients);
        if active_clients.fetch_add(1, Ordering::AcqRel) >= MAX_HTTP_CLIENTS {
            active_clients.fetch_sub(1, Ordering::AcqRel);
            let mut stream = stream;
            let _ = write_response(
                &mut stream,
                503,
                "text/plain; charset=utf-8",
                b"too many clients\n",
            );
            continue;
        }

        let client_hub = hub.clone();
        let client_shutdown = shutdown.clone();
        let client_count = Arc::clone(&active_clients);
        match thread::Builder::new()
            .name("cclover-mon-http-client".to_owned())
            .spawn(move || {
                let result = handle_connection(stream, client_hub, client_shutdown.clone());
                if let Err(error) = result
                    && !client_shutdown.is_requested()
                    && error.kind() != io::ErrorKind::BrokenPipe
                    && error.kind() != io::ErrorKind::ConnectionReset
                {
                    eprintln!("cclover-mon: HTTP client failed: {error}");
                }
                client_count.fetch_sub(1, Ordering::AcqRel);
            }) {
            Ok(thread) => clients.push(thread),
            Err(error) => {
                active_clients.fetch_sub(1, Ordering::AcqRel);
                eprintln!("cclover-mon: failed to spawn HTTP client handler: {error}");
            }
        }
    }

    for client in clients {
        let _ = client.join();
    }
}

fn reap_finished(clients: &mut Vec<JoinHandle<()>>) {
    let mut index = 0;
    while index < clients.len() {
        if clients[index].is_finished() {
            let client = clients.swap_remove(index);
            let _ = client.join();
        } else {
            index += 1;
        }
    }
}

pub(crate) fn wake_address(address: SocketAddr) -> SocketAddr {
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
