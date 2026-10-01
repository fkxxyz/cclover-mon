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
use perf::{PerfRequest, parse_perf, perf};
use probe::probe;
use support::{prepare_pawnio_if_needed, probe_needs_pawnio};
pub use tui::run_tui;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Invocation {
    Launch(LaunchRequest),
    Command(Command),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Command(CommandKind);

#[derive(Clone, Debug, Eq, PartialEq)]
enum CommandKind {
    Dump { samples: u64 },
    Probe { kind: ProbeKind, raw: bool },
    Perf(PerfRequest),
    Help,
}

impl Invocation {
    pub fn requires_terminal(&self) -> bool {
        match self {
            Self::Launch(LaunchRequest::Explicit { tui, .. }) => *tui,
            Self::Launch(LaunchRequest::Auto) => false,
            Self::Command(_) => true,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CliError(String);

impl CliError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

pub fn parse() -> Result<Invocation, CliError> {
    parse_args(std::env::args().skip(1))
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Invocation, CliError> {
    let Some(first) = args.next() else {
        return Ok(Invocation::Launch(LaunchRequest::Auto));
    };

    if first.starts_with('-') && first != "--help" && first != "-h" {
        return parse_launch_options(std::iter::once(first).chain(args)).map(Invocation::Launch);
    }

    let command = match first.as_str() {
        "dump" => CommandKind::Dump {
            samples: parse_dump_samples(args)?,
        },
        "probe" => {
            let collector = args
                .next()
                .ok_or_else(|| CliError::new("probe requires a collector name"))?;
            let kind = collector.parse::<ProbeKind>().map_err(CliError::new)?;
            let raw = match args.next().as_deref() {
                None => false,
                Some("--raw") => true,
                Some(other) => {
                    return Err(CliError::new(format!("unexpected probe argument: {other}")));
                }
            };
            if let Some(extra) = args.next() {
                return Err(CliError::new(format!("unexpected probe argument: {extra}")));
            }
            CommandKind::Probe { kind, raw }
        }
        "perf" => CommandKind::Perf(parse_perf(args)?),
        "help" | "--help" | "-h" => CommandKind::Help,
        other => return Err(CliError::new(format!("unknown command: {other}"))),
    };

    Ok(Invocation::Command(Command(command)))
}

pub fn execute(command: Command) {
    match command.0 {
        CommandKind::Dump { samples } => {
            prepare_pawnio_if_needed(true);
            dump(samples);
        }
        CommandKind::Probe { kind, raw } => {
            prepare_pawnio_if_needed(probe_needs_pawnio(kind));
            probe(kind, raw);
        }
        CommandKind::Perf(request) => perf(request),
        CommandKind::Help => print_help(),
    }
}

pub fn report_error(error: &CliError) {
    eprintln!("cclover-mon: {}\n", error.0);
    print_help();
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

#[cfg(test)]
mod tests;
