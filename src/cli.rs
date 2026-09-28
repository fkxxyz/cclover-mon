use std::net::SocketAddr;
use std::time::{Duration, Instant};

use crate::core::model::CollectionStatus;
use crate::core::{SampleCycle, Sampler};
use crate::platform::{Backend, ProbeKind};
use crate::presentation::{Dashboard, format_bytes, format_percent, format_rate, unavailable};
use crate::runtime::StateSource;
use crate::tui::TerminalUi;
use crate::web::{DEFAULT_HTTP_BIND, HttpConfig};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LaunchOptions {
    pub desktop: bool,
    pub tui: bool,
    pub http: Option<HttpConfig>,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            desktop: true,
            tui: false,
            http: None,
        }
    }
}

pub fn parse() -> Option<LaunchOptions> {
    let mut args = std::env::args().skip(1);
    let Some(first) = args.next() else {
        return Some(LaunchOptions::default());
    };

    if first.starts_with('-') && first != "--help" && first != "-h" {
        return Some(parse_launch_options(std::iter::once(first).chain(args)));
    }

    match first.as_str() {
        "dump" => {
            let samples = parse_dump_samples(args);
            dump(samples);
        }
        "probe" => {
            let collector = args
                .next()
                .unwrap_or_else(|| fail("probe requires a collector name"));
            let kind = collector
                .parse::<ProbeKind>()
                .unwrap_or_else(|error| fail(&error));
            let raw = match args.next().as_deref() {
                None => false,
                Some("--raw") => true,
                Some(other) => fail(&format!("unexpected probe argument: {other}")),
            };
            if let Some(extra) = args.next() {
                fail(&format!("unexpected probe argument: {extra}"));
            }
            probe(kind, raw);
        }
        "perf" => perf(args),
        "help" | "--help" | "-h" => print_help(),
        other => fail(&format!("unknown command: {other}")),
    }

    None
}

fn parse_launch_options(mut args: impl Iterator<Item = String>) -> LaunchOptions {
    let mut desktop = false;
    let mut tui = false;
    let mut http = false;
    let mut bind = DEFAULT_HTTP_BIND;
    let mut bind_explicit = false;
    let mut frontend_explicit = false;

    while let Some(option) = args.next() {
        match option.as_str() {
            "--desktop" => {
                if desktop {
                    fail("--desktop may only be specified once");
                }
                desktop = true;
                frontend_explicit = true;
            }
            "--tui" => {
                if tui {
                    fail("--tui may only be specified once");
                }
                tui = true;
                frontend_explicit = true;
            }
            "--http" => {
                if http {
                    fail("--http may only be specified once");
                }
                http = true;
                frontend_explicit = true;
            }
            "--http-bind" => {
                if bind_explicit {
                    fail("--http-bind may only be specified once");
                }
                let value = args
                    .next()
                    .unwrap_or_else(|| fail("--http-bind requires an IP:port value"));
                bind = value.parse::<SocketAddr>().unwrap_or_else(|_| {
                    fail("--http-bind requires an IP:port value such as 0.0.0.0:9847")
                });
                bind_explicit = true;
            }
            other => fail(&format!("unknown launch option: {other}")),
        }
    }

    if bind_explicit && !http {
        fail("--http-bind requires --http");
    }
    if http && !cfg!(feature = "http") {
        fail("--http is unavailable in this build; rebuild with the `http` feature");
    }

    LaunchOptions {
        desktop: if frontend_explicit { desktop } else { true },
        tui,
        http: http.then_some(HttpConfig { bind }),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PerfLimit {
    Unbounded,
    Duration(Duration),
    Samples(u64),
}

fn parse_dump_samples(mut args: impl Iterator<Item = String>) -> u64 {
    let Some(option) = args.next() else {
        return 2;
    };
    if option != "--samples" {
        fail(&format!(
            "unexpected dump argument: {option}; expected --samples"
        ));
    }
    let samples = args
        .next()
        .unwrap_or_else(|| fail("--samples requires a positive integer"))
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .unwrap_or_else(|| fail("--samples requires a positive integer"));
    if let Some(extra) = args.next() {
        fail(&format!("unexpected dump argument: {extra}"));
    }
    samples.max(2)
}

fn perf(mut args: impl Iterator<Item = String>) {
    let workload = args
        .next()
        .unwrap_or_else(|| fail("perf requires a workload: headless or collector"));

    match workload.as_str() {
        "headless" => {
            let limit = parse_perf_limit(args);
            let mut sampler = Sampler::new(Backend::new());
            run_perf(limit, || {
                std::hint::black_box(sampler.sample());
            });
        }
        "collector" => {
            let collector = args
                .next()
                .unwrap_or_else(|| fail("perf collector requires a collector name"));
            let kind = collector
                .parse::<ProbeKind>()
                .unwrap_or_else(|error| fail(&error));
            let limit = parse_perf_limit(args);
            let mut backend = Backend::new();
            run_perf(limit, || backend.collect_for_perf(kind));
        }
        other => fail(&format!(
            "unknown perf workload {other:?}; expected headless or collector"
        )),
    }
}

fn parse_perf_limit(mut args: impl Iterator<Item = String>) -> PerfLimit {
    let Some(option) = args.next() else {
        return PerfLimit::Unbounded;
    };
    let value = args
        .next()
        .unwrap_or_else(|| fail(&format!("{option} requires a positive integer")));
    if let Some(extra) = args.next() {
        fail(&format!("unexpected perf argument: {extra}"));
    }

    let value = value
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .unwrap_or_else(|| fail(&format!("{option} requires a positive integer")));

    match option.as_str() {
        "--duration" => PerfLimit::Duration(Duration::from_secs(value)),
        "--samples" => PerfLimit::Samples(value),
        _ => fail(&format!(
            "unexpected perf argument: {option}; expected --duration or --samples"
        )),
    }
}

fn run_perf(limit: PerfLimit, mut work: impl FnMut()) {
    let run_started = Instant::now();
    let deadline = match limit {
        PerfLimit::Duration(duration) => Some(run_started + duration),
        PerfLimit::Unbounded | PerfLimit::Samples(_) => None,
    };
    let mut samples = 0_u64;

    loop {
        let cycle = SampleCycle::begin();
        work();
        samples += 1;

        if matches!(limit, PerfLimit::Samples(target) if samples >= target) {
            break;
        }
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            break;
        }

        let remaining_interval = cycle.remaining();
        let sleep_for = deadline.map_or(remaining_interval, |deadline| {
            remaining_interval.min(deadline.saturating_duration_since(Instant::now()))
        });
        if !sleep_for.is_zero() {
            std::thread::sleep(sleep_for);
        }
    }
}

pub fn run_tui(states: StateSource) -> std::io::Result<()> {
    use std::sync::mpsc::TryRecvError;

    let receiver = states.subscribe();
    let mut ui = TerminalUi::enter()?;
    let mut state = receiver.recv().map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::BrokenPipe, "monitor runtime stopped")
    })?;
    ui.draw(Dashboard::new(&state))?;

    loop {
        if ui.wait_for_quit(Duration::from_millis(100))? {
            return Ok(());
        }
        loop {
            match receiver.try_recv() {
                Ok(next) => state = next,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Ok(()),
            }
        }
        ui.draw(Dashboard::new(&state))?;
    }
}

fn dump(samples: u64) {
    let mut sampler = Sampler::new(Backend::new());
    let mut cycle = SampleCycle::begin();
    let mut state = sampler.sample();
    for _ in 1..samples {
        let remaining = cycle.remaining();
        if !remaining.is_zero() {
            std::thread::sleep(remaining);
        }
        cycle = SampleCycle::begin();
        state = sampler.sample();
    }
    let snapshot = state.snapshot;

    println!(
        "CPU: {}",
        snapshot
            .cpu_percent
            .value()
            .copied()
            .map(format_percent)
            .unwrap_or_else(unavailable)
    );
    match snapshot.memory.value() {
        Some(memory) => {
            println!(
                "Memory: {} / {}",
                format_bytes(memory.used_bytes),
                format_bytes(memory.total_bytes)
            );
            println!(
                "Swap: {} / {}",
                format_bytes(memory.swap_used_bytes),
                format_bytes(memory.swap_total_bytes)
            );
        }
        None => {
            println!("Memory: {}", unavailable());
            println!("Swap: {}", unavailable());
        }
    }

    print_section(
        "Top CPU",
        snapshot
            .top_cpu
            .value()
            .into_iter()
            .flatten()
            .map(|item| format!("{}  {}", item.name, format_percent(item.percent))),
    );
    print_section(
        "Top memory",
        snapshot
            .top_memory
            .value()
            .into_iter()
            .flatten()
            .map(|item| format!("{}  {}", item.name, format_bytes(item.bytes))),
    );
    print_section(
        "Network",
        snapshot.networks.value().into_iter().flatten().map(|item| {
            format!(
                "{}  down {}  up {}",
                item.name,
                format_rate(item.down_bytes_per_sec),
                format_rate(item.up_bytes_per_sec)
            )
        }),
    );
    print_section(
        "Disk",
        snapshot
            .disks
            .value()
            .into_iter()
            .flatten()
            .map(|item| format!("{}  {}", item.name, format_rate(item.bytes_per_sec))),
    );
    match snapshot.process_disk_io.value() {
        Some(rows) if rows.is_empty() => println!("Process disk I/O:\n  none"),
        Some(rows) => print_section(
            "Process disk I/O",
            rows.iter().map(|item| {
                format!(
                    "pid={}  {}  read {}  write {}",
                    item.process.pid,
                    item.device,
                    format_rate(item.read_bytes_per_sec),
                    format_rate(item.write_bytes_per_sec)
                )
            }),
        ),
        None => println!("Process disk I/O:\n  {}", unavailable()),
    }
    match snapshot.process_network_io.value() {
        Some(rows) if rows.is_empty() => println!("Process network I/O:\n  none"),
        Some(rows) => print_section(
            "Process network I/O",
            rows.iter().map(|item| {
                format!(
                    "pid={}  {}  rx {}  tx {}",
                    item.process.pid,
                    item.interface,
                    format_rate(item.rx_bytes_per_sec),
                    format_rate(item.tx_bytes_per_sec)
                )
            }),
        ),
        None => println!("Process network I/O:\n  {}", unavailable()),
    }
    print_section(
        "Temperature",
        snapshot
            .temperatures
            .value()
            .into_iter()
            .flatten()
            .map(|item| format!("{}  {:.1}°C", item.name, item.celsius)),
    );
}

fn probe(kind: ProbeKind, raw: bool) {
    let mut backend = Backend::new();
    let started = Instant::now();
    let report = backend.probe(kind);
    let elapsed = started.elapsed();

    println!("collector: {}", kind.as_str());
    println!(
        "status: {}",
        match report.status {
            CollectionStatus::Available => "ok",
            CollectionStatus::Degraded => "degraded",
            CollectionStatus::Unavailable(_) => "unavailable",
        }
    );
    println!("elapsed: {:.3} ms", elapsed.as_secs_f64() * 1_000.0);
    for line in report.summary {
        println!("result: {line}");
    }
    if raw {
        for line in report.raw {
            println!("raw: {line}");
        }
    }
    if report.notes.is_empty() {
        println!("diagnostics: none");
    } else {
        for note in report.notes {
            println!("diagnostic: {note}");
        }
    }
}

fn print_section(title: &str, rows: impl Iterator<Item = String>) {
    println!("{title}:");
    let mut count = 0;
    for row in rows {
        println!("  {row}");
        count += 1;
    }
    if count == 0 {
        println!("  {}", unavailable());
    }
}

fn print_help() {
    println!(
        "cclover-mon\n\n\
         Usage:\n  \
           cclover-mon\n  \
           cclover-mon [--desktop] [--tui] [--http] [--http-bind <ip:port>]\n  \
           cclover-mon dump [--samples <count>]\n  \
           cclover-mon probe <collector> [--raw]\n  \
           cclover-mon perf headless [--duration <seconds> | --samples <count>]\n  \
           cclover-mon perf collector <collector> [--duration <seconds> | --samples <count>]\n\n\
         Frontends:\n  \
           --desktop                 Enable the desktop panel\n  \
           --tui                     Enable the terminal panel\n  \
           --http                    Enable the read-only web panel\n  \
           --http-bind <ip:port>     HTTP listen address (default 127.0.0.1:9847)\n  \
           No frontend flag defaults to --desktop.\n\n\
         Collectors:\n  \
           cpu, memory, processes, network, network-attribution, disk, disk-attribution, temperatures\n\n\
         Development logging:\n  \
           CCLOVER_MON_DEBUG=1 cclover-mon"
    );
}

fn fail(message: &str) -> ! {
    eprintln!("cclover-mon: {message}\n");
    print_help();
    std::process::exit(2);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(args: &[&str]) -> LaunchOptions {
        parse_launch_options(args.iter().map(|arg| (*arg).to_owned()))
    }

    #[test]
    fn no_frontend_flag_defaults_to_desktop() {
        assert_eq!(options(&[]), LaunchOptions::default());
    }

    #[cfg(feature = "http")]
    #[test]
    fn explicit_frontends_are_composable_and_disable_implicit_desktop() {
        assert_eq!(
            options(&["--tui", "--http"]),
            LaunchOptions {
                desktop: false,
                tui: true,
                http: Some(HttpConfig::default()),
            }
        );
        assert_eq!(
            options(&["--desktop", "--tui", "--http"]),
            LaunchOptions {
                desktop: true,
                tui: true,
                http: Some(HttpConfig::default()),
            }
        );
    }

    #[cfg(not(feature = "http"))]
    #[test]
    fn minimal_build_composes_native_frontends_without_http() {
        assert_eq!(
            options(&["--desktop", "--tui"]),
            LaunchOptions {
                desktop: true,
                tui: true,
                http: None,
            }
        );
    }
}
