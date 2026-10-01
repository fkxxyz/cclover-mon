use cclover_runtime::Shutdown;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

pub fn wait_foreground(shutdown: Shutdown) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(unix)]
    {
        return unix::wait(shutdown);
    }
    #[cfg(windows)]
    {
        return windows::wait_foreground(shutdown);
    }
    #[allow(unreachable_code)]
    Err("unsupported server platform".into())
}

#[cfg(windows)]
pub fn run_windows_service(
    config: cclover_http::HttpConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    windows::run_service(config)
}
