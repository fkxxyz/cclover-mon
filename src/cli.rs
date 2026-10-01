use std::net::SocketAddr;
use std::time::{Duration, Instant};

use crate::core::model::{CollectionStatus, CollectionUnavailable};
use crate::core::{SampleCycle, Sampler};
use crate::platform::{Backend, ProbeKind};
use crate::presentation::{Dashboard, format_bytes, format_percent, format_rate, unavailable};
use crate::runtime::StateSource;
use crate::tui::TerminalUi;
use crate::web::{DEFAULT_HTTP_BIND, HttpConfig};

mod dump;
mod launch;
mod perf;
mod probe;
mod support;
mod tui;

use dump::{dump, parse_dump_samples};
pub use launch::LaunchRequest;
use launch::parse_launch_options;
use perf::perf;
use probe::probe;
use support::{prepare_pawnio_if_needed, probe_needs_pawnio};
pub use tui::run_tui;

pub fn parse() -> Option<LaunchRequest> {
    let mut args = std::env::args().skip(1);
    let Some(first) = args.next() else {
        return Some(LaunchRequest::Auto);
    };

    if first.starts_with('-') && first != "--help" && first != "-h" {
        return Some(parse_launch_options(std::iter::once(first).chain(args)));
    }

    match first.as_str() {
        "dump" => {
            let samples = parse_dump_samples(args);
            prepare_pawnio_if_needed(true);
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
            prepare_pawnio_if_needed(probe_needs_pawnio(kind));
            probe(kind, raw);
        }
        "perf" => perf(args),
        "help" | "--help" | "-h" => print_help(),
        other => fail(&format!("unknown command: {other}")),
    }

    None
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
           No frontend flag auto-selects desktop when available, otherwise TUI on an interactive terminal.\n\n\
         Collectors:\n  \
           {}\n\n\
         Development logging:\n  \
           CCLOVER_MON_DEBUG=1 cclover-mon",
        ProbeKind::names_csv()
    );
}

fn fail(message: &str) -> ! {
    eprintln!("cclover-mon: {message}\n");
    print_help();
    std::process::exit(2);
}

#[cfg(test)]
mod tests;
