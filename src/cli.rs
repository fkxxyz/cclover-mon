use std::time::Instant;

use crate::core::{SAMPLE_INTERVAL, Sampler};
use crate::platform::{Backend, ProbeKind};

pub fn run_if_requested() -> bool {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        return false;
    };

    match command.as_str() {
        "dump" => {
            if let Some(extra) = args.next() {
                fail(&format!("unexpected argument for dump: {extra}"));
            }
            dump();
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
        "help" | "--help" | "-h" => print_help(),
        other => fail(&format!("unknown command: {other}")),
    }

    true
}

fn dump() {
    let mut sampler = Sampler::new(Backend::new());
    let started = Instant::now();
    sampler.sample();
    let remaining = SAMPLE_INTERVAL.saturating_sub(started.elapsed());
    if !remaining.is_zero() {
        std::thread::sleep(remaining);
    }
    let state = sampler.sample();
    let snapshot = state.snapshot;

    println!("CPU: {}", percent(snapshot.cpu_percent));
    match snapshot.memory {
        Some(memory) => {
            println!(
                "Memory: {} / {}",
                bytes(memory.used_bytes),
                bytes(memory.total_bytes)
            );
            println!(
                "Swap: {} / {}",
                bytes(memory.swap_used_bytes),
                bytes(memory.swap_total_bytes)
            );
        }
        None => {
            println!("Memory: unavailable");
            println!("Swap: unavailable");
        }
    }

    print_section(
        "Top CPU",
        snapshot
            .top_cpu
            .iter()
            .map(|item| format!("{}  {:.1}%", item.name, item.percent)),
    );
    print_section(
        "Top memory",
        snapshot
            .top_memory
            .iter()
            .map(|item| format!("{}  {}", item.name, bytes(item.bytes))),
    );
    print_section(
        "Network",
        snapshot.networks.iter().map(|item| {
            format!(
                "{}  down {}/s  up {}/s",
                item.name,
                bytes(item.down_bytes_per_sec as u64),
                bytes(item.up_bytes_per_sec as u64)
            )
        }),
    );
    print_section(
        "Disk",
        snapshot
            .disks
            .iter()
            .map(|item| format!("{}  {}/s", item.name, bytes(item.bytes_per_sec as u64))),
    );
    print_section(
        "Temperature",
        snapshot
            .temperatures
            .iter()
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
        if report.available {
            "ok"
        } else {
            "unavailable"
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
        println!("  unavailable");
    }
}

fn percent(value: Option<f64>) -> String {
    value.map_or_else(|| "unavailable".to_owned(), |value| format!("{value:.1}%"))
}

fn bytes(value: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    let value = value as f64;
    if value >= GIB {
        format!("{:.1} GiB", value / GIB)
    } else if value >= MIB {
        format!("{:.1} MiB", value / MIB)
    } else if value >= KIB {
        format!("{:.1} KiB", value / KIB)
    } else {
        format!("{value:.0} B")
    }
}

fn print_help() {
    println!(
        "cclover-mon\n\n\
         Usage:\n  \
           cclover-mon\n  \
           cclover-mon dump\n  \
           cclover-mon probe <collector> [--raw]\n\n\
         Collectors:\n  \
           cpu, memory, processes, network, disk, temperatures\n\n\
         Development logging:\n  \
           CCLOVER_MON_DEBUG=1 cclover-mon"
    );
}

fn fail(message: &str) -> ! {
    eprintln!("cclover-mon: {message}\n");
    print_help();
    std::process::exit(2);
}
