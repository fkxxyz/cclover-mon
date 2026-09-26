use std::fmt;
use std::sync::OnceLock;
use std::time::Instant;

pub fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var_os("CCLOVER_MON_DEBUG").is_some_and(|value| !value.is_empty() && value != "0")
    })
}

pub fn log(message: fmt::Arguments<'_>) {
    if enabled() {
        eprintln!("[cclover-mon] {message}");
    }
}

pub fn timed<T>(label: &str, work: impl FnOnce() -> T) -> T {
    if !enabled() {
        return work();
    }

    let started = Instant::now();
    let result = work();
    log(format_args!(
        "{label} duration={:.3}ms",
        started.elapsed().as_secs_f64() * 1_000.0
    ));
    result
}
