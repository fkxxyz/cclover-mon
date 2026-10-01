#![allow(unsafe_code)]

use std::io;
use std::ptr;
use std::sync::OnceLock;

use cclover_http::{HttpConfig, HttpServer};
use cclover_runtime::{NativeRuntime, Shutdown};
use windows_sys::Win32::System::Services::{
    RegisterServiceCtrlHandlerW, SERVICE_ACCEPT_SHUTDOWN, SERVICE_ACCEPT_STOP,
    SERVICE_CONTROL_SHUTDOWN, SERVICE_CONTROL_STOP, SERVICE_RUNNING, SERVICE_START_PENDING,
    SERVICE_STATUS, SERVICE_STATUS_HANDLE, SERVICE_STOP_PENDING, SERVICE_STOPPED,
    SERVICE_TABLE_ENTRYW, SERVICE_WIN32_OWN_PROCESS, SetServiceStatus, StartServiceCtrlDispatcherW,
};

const SERVICE_NAME: &str = "cclover-mon-server";
static SERVICE_CONFIG: OnceLock<HttpConfig> = OnceLock::new();
static SERVICE_SHUTDOWN: OnceLock<Shutdown> = OnceLock::new();

pub fn wait_foreground(shutdown: Shutdown) -> Result<(), Box<dyn std::error::Error>> {
    let signal = shutdown.clone();
    ctrlc::set_handler(move || signal.request())?;
    shutdown.wait();
    Ok(())
}

pub fn run_service(config: HttpConfig) -> Result<(), Box<dyn std::error::Error>> {
    SERVICE_CONFIG
        .set(config)
        .map_err(|_| "Windows service configuration already initialized")?;
    SERVICE_SHUTDOWN
        .set(Shutdown::default())
        .map_err(|_| "Windows service shutdown already initialized")?;

    let mut name: Vec<u16> = SERVICE_NAME.encode_utf16().chain([0]).collect();
    let table = [
        SERVICE_TABLE_ENTRYW {
            lpServiceName: name.as_mut_ptr(),
            lpServiceProc: Some(service_main),
        },
        SERVICE_TABLE_ENTRYW {
            lpServiceName: ptr::null_mut(),
            lpServiceProc: None,
        },
    ];

    if unsafe { StartServiceCtrlDispatcherW(table.as_ptr()) } == 0 {
        return Err(io::Error::last_os_error().into());
    }
    Ok(())
}

unsafe extern "system" fn service_main(_argc: u32, _argv: *mut *mut u16) {
    if let Err(error) = service_main_inner() {
        eprintln!("cclover-mon-server: Windows service failed: {error}");
    }
}

fn service_main_inner() -> Result<(), Box<dyn std::error::Error>> {
    let name: Vec<u16> = SERVICE_NAME.encode_utf16().chain([0]).collect();
    let status_handle =
        unsafe { RegisterServiceCtrlHandlerW(name.as_ptr(), Some(service_control)) };
    if status_handle.is_null() {
        return Err(io::Error::last_os_error().into());
    }

    set_status(status_handle, SERVICE_START_PENDING, 0, 1, 10_000)?;

    let shutdown = SERVICE_SHUTDOWN
        .get()
        .expect("service shutdown initialized before dispatcher")
        .clone();
    let config = *SERVICE_CONFIG
        .get()
        .expect("service configuration initialized before dispatcher");

    let runtime = match NativeRuntime::start_with_shutdown(shutdown.clone()) {
        Ok(runtime) => runtime,
        Err(error) => {
            set_status(status_handle, SERVICE_STOPPED, 0, 0, 0)?;
            return Err(error.into());
        }
    };
    let http = match HttpServer::start(config, runtime.states(), shutdown.clone()) {
        Ok(http) => http,
        Err(error) => {
            drop(runtime);
            set_status(status_handle, SERVICE_STOPPED, 0, 0, 0)?;
            return Err(error.into());
        }
    };

    set_status(
        status_handle,
        SERVICE_RUNNING,
        SERVICE_ACCEPT_STOP | SERVICE_ACCEPT_SHUTDOWN,
        0,
        0,
    )?;

    shutdown.wait();
    set_status(status_handle, SERVICE_STOP_PENDING, 0, 1, 10_000)?;
    drop(http);
    drop(runtime);
    set_status(status_handle, SERVICE_STOPPED, 0, 0, 0)?;
    Ok(())
}

unsafe extern "system" fn service_control(control: u32) {
    if matches!(control, SERVICE_CONTROL_STOP | SERVICE_CONTROL_SHUTDOWN)
        && let Some(shutdown) = SERVICE_SHUTDOWN.get()
    {
        shutdown.request();
    }
}

fn set_status(
    handle: SERVICE_STATUS_HANDLE,
    state: u32,
    controls: u32,
    checkpoint: u32,
    wait_hint_ms: u32,
) -> io::Result<()> {
    let status = SERVICE_STATUS {
        dwServiceType: SERVICE_WIN32_OWN_PROCESS,
        dwCurrentState: state,
        dwControlsAccepted: controls,
        dwWin32ExitCode: 0,
        dwServiceSpecificExitCode: 0,
        dwCheckPoint: checkpoint,
        dwWaitHint: wait_hint_ms,
    };
    if unsafe { SetServiceStatus(handle, &status) } == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
