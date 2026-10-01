use super::*;
pub(super) fn parse_dump_samples(mut args: impl Iterator<Item = String>) -> u64 {
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

pub(super) fn dump(samples: u64) {
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
        snapshot.disks.value().into_iter().flatten().map(|item| {
            format!(
                "{}  {}",
                item.metadata.system_label,
                format_rate(item.bytes_per_sec)
            )
        }),
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
    print_section(
        "Fans",
        snapshot
            .fans
            .value()
            .into_iter()
            .flatten()
            .map(|item| format!("{}  {} RPM", item.name, item.rpm)),
    );
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
