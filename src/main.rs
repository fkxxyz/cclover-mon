#![deny(unsafe_code)]

use cclover_mon::{app, cli, platform, web};

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(options) = cli::parse() else {
        return Ok(());
    };
    let http = options.http.map(web::HttpServer::start).transpose()?;
    let web_state = http.as_ref().map(web::HttpServer::state_hub);

    platform::desktop::run(app::DesktopApp::new(web_state))?;
    Ok(())
}

#[cfg(test)]
mod architecture_tests {
    #[test]
    fn execution_adapters_do_not_own_sampling_interval_arithmetic() {
        for (path, source) in [
            ("src/app.rs", include_str!("app.rs")),
            ("src/cli.rs", include_str!("cli.rs")),
        ] {
            assert!(
                !source.contains("SAMPLE_INTERVAL"),
                "{path} must consume core sampling policy through SampleCycle instead of depending on SAMPLE_INTERVAL directly"
            );
        }
    }
}
