#![deny(unsafe_code)]
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use cclover_mon::{cli, runtime, web};

#[cfg(target_os = "windows")]
mod windows_console;

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "windows")]
    windows_console::prepare();

    #[cfg(target_os = "windows")]
    if let Some(code) = cclover_mon::platform::early_command_exit_code() {
        std::process::exit(code);
    }

    let Some(options) = cli::parse() else {
        return Ok(());
    };

    #[cfg(target_os = "windows")]
    cclover_mon::platform::prepare_machine_capability();

    let http = options.http.map(web::HttpServer::start).transpose()?;
    let web_state = http.as_ref().map(web::HttpServer::state_hub);
    let runtime = runtime::NativeRuntime::start(web_state)?;

    if options.desktop {
        let tui_thread = options.tui.then(|| {
            let states = runtime.states();
            std::thread::Builder::new()
                .name("cclover-mon-tui".to_owned())
                .spawn(move || cli::run_tui(states))
                .expect("failed to spawn TUI thread")
        });

        cclover_desktop::run(cclover_desktop::DesktopApp::new(
            runtime.states().subscribe(),
        ))?;
        drop(runtime);
        if let Some(thread) = tui_thread {
            thread.join().map_err(|_| "TUI thread panicked")??;
        }
        return Ok(());
    }

    if options.tui {
        cli::run_tui(runtime.states())?;
        return Ok(());
    }

    loop {
        std::thread::park();
    }
}

#[cfg(test)]
mod architecture_tests {
    #[test]
    fn execution_adapters_do_not_own_sampling_interval_arithmetic() {
        for (path, source) in [
            ("src/cli.rs", include_str!("cli.rs")),
            ("src/runtime.rs", include_str!("runtime.rs")),
            ("src/tui.rs", include_str!("tui.rs")),
        ] {
            assert!(
                !source.contains("SAMPLE_INTERVAL"),
                "{path} must consume core sampling policy through SampleCycle instead of depending on SAMPLE_INTERVAL directly"
            );
        }
    }
}
