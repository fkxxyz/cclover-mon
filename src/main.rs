#![deny(unsafe_code)]

use cclover_mon::{app, cli, platform, runtime, web};

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(options) = cli::parse() else {
        return Ok(());
    };

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

        platform::desktop::run(app::DesktopApp::new(runtime.states()))?;
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
            ("src/app.rs", include_str!("app.rs")),
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
