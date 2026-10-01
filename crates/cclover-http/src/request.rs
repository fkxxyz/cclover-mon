use std::io::{self, Read, Write};
use std::net::TcpStream;

pub(super) fn write_not_ready(stream: &mut TcpStream) -> io::Result<()> {
    write_response(
        stream,
        503,
        "text/plain; charset=utf-8",
        b"monitor state not ready\n",
    )
}

pub(super) fn write_json_response(stream: &mut TcpStream, body: &[u8]) -> io::Result<()> {
    write_response(stream, 200, "application/json", body)
}

pub(super) fn read_request(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
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

pub(super) fn parse_request_line(request: &[u8]) -> Option<(&str, &str)> {
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

pub(super) fn write_response(
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
        503 => "Service Unavailable",
        _ => "Error",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'self'; script-src 'self'; connect-src 'self'; style-src 'unsafe-inline'; object-src 'none'\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    stream.flush()
}
