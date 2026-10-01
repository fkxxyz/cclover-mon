use cclover_runtime::Shutdown;
use signal_hook::consts::signal::{SIGINT, SIGTERM};
use signal_hook::iterator::Signals;

pub fn wait(shutdown: Shutdown) -> Result<(), Box<dyn std::error::Error>> {
    let mut signals = Signals::new([SIGINT, SIGTERM])?;
    if signals.forever().next().is_some() {
        shutdown.request();
    }
    Ok(())
}
