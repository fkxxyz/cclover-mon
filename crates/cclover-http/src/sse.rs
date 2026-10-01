use std::io::{self, Write};
use std::net::TcpStream;
use std::time::Duration;

use cclover_runtime::Shutdown;
use crossbeam_channel::{RecvError, after, select};

use crate::state::StateHub;

pub(super) fn stream_events(
    mut stream: TcpStream,
    hub: StateHub,
    shutdown: Shutdown,
) -> io::Result<()> {
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nX-Accel-Buffering: no\r\nX-Content-Type-Options: nosniff\r\nConnection: keep-alive\r\n\r\n"
    )?;

    let (latest, receiver) = hub.subscribe();
    if let Some(latest) = latest {
        write_event(&mut stream, &latest)?;
    }
    let shutdown_receiver = shutdown.subscribe();
    loop {
        let keepalive = after(Duration::from_secs(15));
        select! {
            recv(shutdown_receiver) -> _ => break,
            recv(receiver) -> state => match state {
                Ok(state) => write_event(&mut stream, &state)?,
                Err(RecvError) => break,
            },
            recv(keepalive) -> _ => {
                stream.write_all(b": keepalive\n\n")?;
                stream.flush()?;
            }
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
