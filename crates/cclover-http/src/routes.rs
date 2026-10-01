use std::io;
use std::net::TcpStream;
use std::time::Duration;

use cclover_runtime::Shutdown;

use crate::api::ApiV1Slice;
use crate::assets::{BOOTSTRAP_JS, INDEX_HTML};
use crate::request::{
    parse_request_line, read_request, write_json_response, write_not_ready, write_response,
};
use crate::sse::stream_events;
use crate::state::StateHub;

pub(super) fn handle_connection(
    mut stream: TcpStream,
    hub: StateHub,
    shutdown: Shutdown,
) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
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
        "/style.css" => {
            let stylesheet = cclover_web_ui::stylesheet();
            write_response(
                &mut stream,
                200,
                "text/css; charset=utf-8",
                stylesheet.as_bytes(),
            )
        }
        "/healthz" => write_response(&mut stream, 200, "text/plain; charset=utf-8", b"ok\n"),
        "/readyz" => {
            if hub.is_ready() {
                write_response(&mut stream, 200, "text/plain; charset=utf-8", b"ready\n")
            } else {
                write_not_ready(&mut stream)
            }
        }
        "/api/v1/state" => match hub.latest_api_v1_json() {
            Some(state) => write_json_response(&mut stream, state.as_bytes()),
            None => write_not_ready(&mut stream),
        },
        "/api/v1/cpu" => write_api_slice(&mut stream, &hub, ApiV1Slice::Cpu),
        "/api/v1/memory" => write_api_slice(&mut stream, &hub, ApiV1Slice::Memory),
        "/api/v1/disks" => write_api_slice(&mut stream, &hub, ApiV1Slice::Disks),
        "/api/v1/networks" => write_api_slice(&mut stream, &hub, ApiV1Slice::Networks),
        "/api/v1/temperatures" => write_api_slice(&mut stream, &hub, ApiV1Slice::Temperatures),
        "/api/v1/fans" => write_api_slice(&mut stream, &hub, ApiV1Slice::Fans),
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
        "/api/v1/history/fans" => write_api_slice(&mut stream, &hub, ApiV1Slice::HistoryFans),
        "/events" => stream_events(stream, hub, shutdown),
        _ => write_response(
            &mut stream,
            404,
            "text/plain; charset=utf-8",
            b"not found\n",
        ),
    }
}

fn write_api_slice(stream: &mut TcpStream, hub: &StateHub, slice: ApiV1Slice) -> io::Result<()> {
    let Some(state) = hub.latest_api_v1() else {
        return write_not_ready(stream);
    };
    match state.serialize_slice(slice) {
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
