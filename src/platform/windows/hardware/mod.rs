mod intel;
#[cfg(target_arch = "x86_64")]
mod superio;

use std::time::{Duration, Instant};

use crate::core::model::{Collection, CollectionUnavailable, FanSnapshot, TemperatureSnapshot};

use super::diagnostics::report_issue;
use super::pawnio::{OpenError, Session};
use super::provision::{self, MachineStatus};

const INTEL_MSR_MODULE: &[u8] = include_bytes!(env!("CCLOVER_PAWNIO_INTEL_MSR_BIN"));
#[cfg(target_arch = "x86_64")]
const LPC_IO_MODULE: &[u8] = include_bytes!(env!("CCLOVER_PAWNIO_LPC_IO_BIN"));
const RETRY_INTERVAL: Duration = Duration::from_secs(30);

pub(super) struct Collector {
    state: State,
}

pub(super) struct Batch {
    pub temperatures: Collection<Vec<TemperatureSnapshot>>,
    pub fans: Collection<Vec<FanSnapshot>>,
}

enum State {
    Uninitialized,
    Ready(Runtime),
    Retry {
        at: Instant,
        reason: CollectionUnavailable,
    },
    Unsupported,
    StableFailure(CollectionUnavailable),
}

struct Runtime {
    intel_msr: SourceSession,
    #[cfg(target_arch = "x86_64")]
    lpc_io: SourceSession,
    #[cfg(target_arch = "x86_64")]
    superio: superio::Collector,
}

enum SourceSession {
    Ready(Session),
    Unavailable(CollectionUnavailable),
}

impl Collector {
    pub(super) fn new() -> Self {
        Self {
            state: State::Uninitialized,
        }
    }

    pub(super) fn collect(&mut self, notes: Option<&mut Vec<String>>) -> Batch {
        if matches!(self.state, State::Uninitialized) {
            self.state = initialize_state();
        } else if let State::Retry { at, .. } = self.state
            && Instant::now() >= at
        {
            self.state = initialize_state();
        }

        match &mut self.state {
            State::Uninitialized => unreachable!("hardware telemetry initialized above"),
            State::Ready(runtime) => runtime.collect(notes),
            State::Retry { reason, .. } | State::StableFailure(reason) => {
                Batch::unavailable(*reason)
            }
            State::Unsupported => Batch::unavailable(CollectionUnavailable::Unsupported),
        }
    }
}

impl Default for Collector {
    fn default() -> Self {
        Self::new()
    }
}

impl Runtime {
    fn collect(&mut self, mut notes: Option<&mut Vec<String>>) -> Batch {
        let temperatures = match &self.intel_msr {
            SourceSession::Ready(session) => match intel::collect(session) {
                Ok(value) => Collection::available(vec![value]),
                Err(error) => {
                    report_issue(&mut notes, || {
                        format!("PawnIO Intel package temperature failed: {error}")
                    });
                    Collection::unavailable(CollectionUnavailable::Unavailable)
                }
            },
            SourceSession::Unavailable(reason) => Collection::unavailable(*reason),
        };

        #[cfg(target_arch = "x86_64")]
        let fans = match &self.lpc_io {
            SourceSession::Ready(session) => self.superio.collect(session, notes),
            SourceSession::Unavailable(reason) => Collection::unavailable(*reason),
        };
        #[cfg(not(target_arch = "x86_64"))]
        let fans = Collection::unavailable(CollectionUnavailable::Unsupported);

        Batch { temperatures, fans }
    }
}

impl Batch {
    fn unavailable(reason: CollectionUnavailable) -> Self {
        Self {
            temperatures: Collection::unavailable(reason),
            fans: Collection::unavailable(reason),
        }
    }
}

#[derive(Debug)]
enum InitFailure {
    Unsupported,
    Retry(CollectionUnavailable),
    Stable(CollectionUnavailable),
}

fn initialize() -> Result<Runtime, InitFailure> {
    let intel_msr = open_module_session(INTEL_MSR_MODULE)?;
    #[cfg(target_arch = "x86_64")]
    let lpc_io = open_module_session(LPC_IO_MODULE)?;

    Ok(Runtime {
        intel_msr,
        #[cfg(target_arch = "x86_64")]
        lpc_io,
        #[cfg(target_arch = "x86_64")]
        superio: superio::Collector::new(),
    })
}

fn open_session() -> Result<Session, InitFailure> {
    match Session::open() {
        Ok(session) => Ok(session),
        Err(OpenError::NotInstalled) => match provision::machine_status() {
            Some(MachineStatus::Ready) => {
                Session::open().map_err(|_| InitFailure::Retry(CollectionUnavailable::Unavailable))
            }
            Some(MachineStatus::Unsupported) => Err(InitFailure::Unsupported),
            #[cfg(not(target_arch = "x86_64"))]
            Some(MachineStatus::ProvisioningUnsupported) => Err(InitFailure::Unsupported),
            #[cfg(target_arch = "x86_64")]
            Some(MachineStatus::ElevationDeclined) => {
                Err(InitFailure::Stable(CollectionUnavailable::PermissionDenied))
            }
            #[cfg(target_arch = "x86_64")]
            Some(MachineStatus::RebootRequired) => {
                Err(InitFailure::Stable(CollectionUnavailable::Unavailable))
            }
            Some(MachineStatus::InstallFailed) | None => {
                Err(InitFailure::Stable(CollectionUnavailable::Unavailable))
            }
        },
        Err(OpenError::PermissionDenied) => {
            Err(InitFailure::Retry(CollectionUnavailable::PermissionDenied))
        }
        Err(OpenError::Other) => Err(InitFailure::Retry(CollectionUnavailable::Unavailable)),
    }
}

fn open_module_session(module: &[u8]) -> Result<SourceSession, InitFailure> {
    let session = open_session()?;
    Ok(match session.load_module(module) {
        Ok(()) => SourceSession::Ready(session),
        Err(error) => match error.raw_os_error() {
            Some(50) => SourceSession::Unavailable(CollectionUnavailable::Unsupported),
            Some(5) => SourceSession::Unavailable(CollectionUnavailable::PermissionDenied),
            _ => SourceSession::Unavailable(CollectionUnavailable::Unavailable),
        },
    })
}

fn initialize_state() -> State {
    match initialize() {
        Ok(runtime) => State::Ready(runtime),
        Err(InitFailure::Unsupported) => State::Unsupported,
        Err(InitFailure::Retry(reason)) => State::Retry {
            at: Instant::now() + RETRY_INTERVAL,
            reason,
        },
        Err(InitFailure::Stable(reason)) => State::StableFailure(reason),
    }
}
