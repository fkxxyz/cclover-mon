use cclover_runtime::Shutdown;

#[cfg(target_os = "linux")]
pub fn wait(shutdown: Shutdown) -> Result<(), Box<dyn std::error::Error>> {
    use signal_hook::consts::signal::{SIGINT, SIGTERM};
    use signal_hook::iterator::Signals;

    let mut signals = Signals::new([SIGINT, SIGTERM])?;
    if signals.forever().next().is_some() {
        shutdown.request();
    }
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn wait(shutdown: Shutdown) -> Result<(), Box<dyn std::error::Error>> {
    let signal = shutdown.clone();
    ctrlc::set_handler(move || signal.request())?;
    shutdown.wait();
    Ok(())
}
