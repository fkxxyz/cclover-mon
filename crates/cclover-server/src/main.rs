#![deny(unsafe_code)]

mod cli;
mod lifecycle;

use cclover_http::{HttpConfig, HttpServer};
use cclover_runtime::{NativeRuntime, Shutdown};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let command = cli::parse().map_err(|error| format!("cclover-mon-server: {error}"))?;
    match command {
        cli::Command::Help => {
            cli::print_help();
            Ok(())
        }
        cli::Command::Run(config) => run_foreground(config),
        cli::Command::Service(config) => run_service(config),
    }
}

fn run_foreground(config: HttpConfig) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "windows")]
    cclover_platform::prepare_machine_capability();

    let shutdown = Shutdown::default();
    let runtime = NativeRuntime::start_with_shutdown(shutdown.clone())?;
    let _http = HttpServer::start(config, runtime.states(), shutdown.clone())?;
    lifecycle::wait_foreground(shutdown)?;
    drop(runtime);
    Ok(())
}

fn run_service(config: HttpConfig) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    {
        cclover_platform::prepare_machine_capability();
        return lifecycle::run_windows_service(config);
    }
    #[cfg(not(windows))]
    {
        let _ = config;
        Err("--service is only supported on Windows".into())
    }
}
