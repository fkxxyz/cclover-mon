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
    intel_msr: Source<Session>,
    #[cfg(target_arch = "x86_64")]
    superio: Source<SuperIoRuntime>,
}

pub(super) struct Batch {
    pub temperatures: Collection<Vec<TemperatureSnapshot>>,
    pub fans: Collection<Vec<FanSnapshot>>,
}

struct Source<T> {
    state: SourceState<T>,
}

enum SourceState<T> {
    Uninitialized,
    Ready(T),
    Retry {
        at: Instant,
        reason: CollectionUnavailable,
    },
    StableUnavailable(CollectionUnavailable),
}

#[cfg(target_arch = "x86_64")]
struct SuperIoRuntime {
    session: Session,
    collector: superio::Collector,
}

#[derive(Debug)]
enum InitFailure {
    Retry(CollectionUnavailable),
    Stable(CollectionUnavailable),
}

impl Collector {
    pub(super) fn new() -> Self {
        Self {
            intel_msr: Source::new(),
            #[cfg(target_arch = "x86_64")]
            superio: Source::new(),
        }
    }

    pub(super) fn collect(&mut self, mut notes: Option<&mut Vec<String>>) -> Batch {
        let now = Instant::now();
        let temperatures = self.collect_temperatures_at(now, notes.as_deref_mut());
        let fans = self.collect_fans_at(now, notes);
        Batch { temperatures, fans }
    }

    pub(super) fn collect_temperatures(
        &mut self,
        notes: Option<&mut Vec<String>>,
    ) -> Collection<Vec<TemperatureSnapshot>> {
        self.collect_temperatures_at(Instant::now(), notes)
    }

    pub(super) fn collect_fans(
        &mut self,
        notes: Option<&mut Vec<String>>,
    ) -> Collection<Vec<FanSnapshot>> {
        self.collect_fans_at(Instant::now(), notes)
    }

    fn collect_temperatures_at(
        &mut self,
        now: Instant,
        mut notes: Option<&mut Vec<String>>,
    ) -> Collection<Vec<TemperatureSnapshot>> {
        if let Err(reason) = self
            .intel_msr
            .ensure_ready(now, || open_module_session(INTEL_MSR_MODULE))
        {
            return Collection::unavailable(reason);
        }

        let result = intel::collect(
            self.intel_msr
                .ready_mut()
                .expect("Intel MSR source is ready after ensure_ready"),
        );
        match result {
            Ok(value) => Collection::available(vec![value]),
            Err(error) => {
                report_issue(&mut notes, || {
                    format!("PawnIO Intel package temperature failed: {error}")
                });
                if source_transport_failed(&error) {
                    self.intel_msr
                        .retry(now, CollectionUnavailable::Unavailable);
                }
                Collection::unavailable(CollectionUnavailable::Unavailable)
            }
        }
    }

    #[cfg(target_arch = "x86_64")]
    fn collect_fans_at(
        &mut self,
        now: Instant,
        notes: Option<&mut Vec<String>>,
    ) -> Collection<Vec<FanSnapshot>> {
        if let Err(reason) = self.superio.ensure_ready(now, initialize_superio) {
            return Collection::unavailable(reason);
        }

        let runtime = self
            .superio
            .ready_mut()
            .expect("Super-I/O source is ready after ensure_ready");
        match runtime.collector.collect(&runtime.session, notes) {
            superio::CollectOutcome::Observation(collection) => collection,
            superio::CollectOutcome::SourceFailed(error) => {
                let reason = if error.kind() == std::io::ErrorKind::PermissionDenied {
                    CollectionUnavailable::PermissionDenied
                } else {
                    CollectionUnavailable::Unavailable
                };
                self.superio.retry(now, reason);
                Collection::unavailable(reason)
            }
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn collect_fans_at(
        &mut self,
        _now: Instant,
        _notes: Option<&mut Vec<String>>,
    ) -> Collection<Vec<FanSnapshot>> {
        Collection::unavailable(CollectionUnavailable::Unsupported)
    }
}

impl Default for Collector {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Source<T> {
    fn new() -> Self {
        Self {
            state: SourceState::Uninitialized,
        }
    }

    fn ensure_ready(
        &mut self,
        now: Instant,
        initialize: impl FnOnce() -> Result<T, InitFailure>,
    ) -> Result<(), CollectionUnavailable> {
        let should_initialize = match &self.state {
            SourceState::Uninitialized => true,
            SourceState::Retry { at, .. } => now >= *at,
            SourceState::Ready(_) | SourceState::StableUnavailable(_) => false,
        };

        if should_initialize {
            self.state = match initialize() {
                Ok(runtime) => SourceState::Ready(runtime),
                Err(InitFailure::Retry(reason)) => SourceState::Retry {
                    at: now + RETRY_INTERVAL,
                    reason,
                },
                Err(InitFailure::Stable(reason)) => SourceState::StableUnavailable(reason),
            };
        }

        match &self.state {
            SourceState::Ready(_) => Ok(()),
            SourceState::Retry { reason, .. } | SourceState::StableUnavailable(reason) => {
                Err(*reason)
            }
            SourceState::Uninitialized => unreachable!("source initialization handled above"),
        }
    }

    fn ready_mut(&mut self) -> Option<&mut T> {
        match &mut self.state {
            SourceState::Ready(runtime) => Some(runtime),
            _ => None,
        }
    }

    fn retry(&mut self, now: Instant, reason: CollectionUnavailable) {
        self.state = SourceState::Retry {
            at: now + RETRY_INTERVAL,
            reason,
        };
    }
}

#[cfg(target_arch = "x86_64")]
impl SuperIoRuntime {
    fn new(session: Session) -> Self {
        Self {
            session,
            collector: superio::Collector::new(),
        }
    }
}

fn open_session() -> Result<Session, InitFailure> {
    match Session::open() {
        Ok(session) => Ok(session),
        Err(OpenError::NotInstalled) => match provision::machine_status() {
            Some(MachineStatus::Ready) => {
                Session::open().map_err(|_| InitFailure::Retry(CollectionUnavailable::Unavailable))
            }
            Some(MachineStatus::Unsupported) => {
                Err(InitFailure::Stable(CollectionUnavailable::Unsupported))
            }
            #[cfg(not(target_arch = "x86_64"))]
            Some(MachineStatus::ProvisioningUnsupported) => {
                Err(InitFailure::Stable(CollectionUnavailable::Unsupported))
            }
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

fn open_module_session(module: &[u8]) -> Result<Session, InitFailure> {
    let session = open_session()?;
    match session.load_module(module) {
        Ok(()) => Ok(session),
        Err(error) => match error.raw_os_error() {
            Some(50) => Err(InitFailure::Stable(CollectionUnavailable::Unsupported)),
            Some(5) => Err(InitFailure::Stable(CollectionUnavailable::PermissionDenied)),
            _ => Err(InitFailure::Retry(CollectionUnavailable::Unavailable)),
        },
    }
}

#[cfg(target_arch = "x86_64")]
fn initialize_superio() -> Result<SuperIoRuntime, InitFailure> {
    open_module_session(LPC_IO_MODULE).map(SuperIoRuntime::new)
}

fn source_transport_failed(error: &std::io::Error) -> bool {
    error.raw_os_error().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_initializes_independently() {
        let now = Instant::now();
        let mut source = Source::new();
        let mut calls = 0;

        assert_eq!(
            source.ensure_ready(now, || {
                calls += 1;
                Ok::<_, InitFailure>(17_u8)
            }),
            Ok(())
        );
        assert_eq!(calls, 1);
        assert_eq!(source.ready_mut().copied(), Some(17));
    }

    #[test]
    fn retry_waits_for_deadline_then_reinitializes_only_that_source() {
        let now = Instant::now();
        let mut source = Source::<u8>::new();
        let mut calls = 0;

        assert_eq!(
            source.ensure_ready(now, || {
                calls += 1;
                Err(InitFailure::Retry(CollectionUnavailable::Unavailable))
            }),
            Err(CollectionUnavailable::Unavailable)
        );
        assert_eq!(calls, 1);

        assert_eq!(
            source.ensure_ready(now + RETRY_INTERVAL - Duration::from_millis(1), || {
                calls += 1;
                Ok(7)
            }),
            Err(CollectionUnavailable::Unavailable)
        );
        assert_eq!(calls, 1);

        assert_eq!(
            source.ensure_ready(now + RETRY_INTERVAL, || {
                calls += 1;
                Ok(7)
            }),
            Ok(())
        );
        assert_eq!(calls, 2);
        assert_eq!(source.ready_mut().copied(), Some(7));
    }

    #[test]
    fn stable_unavailability_never_reinitializes() {
        let now = Instant::now();
        let mut source = Source::<u8>::new();
        let mut calls = 0;

        assert_eq!(
            source.ensure_ready(now, || {
                calls += 1;
                Err(InitFailure::Stable(CollectionUnavailable::Unsupported))
            }),
            Err(CollectionUnavailable::Unsupported)
        );
        assert_eq!(
            source.ensure_ready(now + Duration::from_secs(300), || {
                calls += 1;
                Ok(1)
            }),
            Err(CollectionUnavailable::Unsupported)
        );
        assert_eq!(calls, 1);
    }

    #[test]
    fn one_source_failure_does_not_change_another_source() {
        let now = Instant::now();
        let mut failed = Source::<u8>::new();
        let mut ready = Source::<u8>::new();

        assert_eq!(
            failed.ensure_ready(now, || {
                Err(InitFailure::Retry(CollectionUnavailable::Unavailable))
            }),
            Err(CollectionUnavailable::Unavailable)
        );
        assert_eq!(ready.ensure_ready(now, || Ok(9)), Ok(()));
        assert_eq!(ready.ready_mut().copied(), Some(9));
    }

    #[test]
    fn runtime_failure_retries_only_failed_source() {
        let now = Instant::now();
        let mut failed = Source::new();
        let mut ready = Source::new();
        assert_eq!(
            failed.ensure_ready(now, || Ok::<_, InitFailure>(1_u8)),
            Ok(())
        );
        assert_eq!(
            ready.ensure_ready(now, || Ok::<_, InitFailure>(2_u8)),
            Ok(())
        );

        failed.retry(now, CollectionUnavailable::Unavailable);

        assert_eq!(
            failed.ensure_ready(now, || Ok(3)),
            Err(CollectionUnavailable::Unavailable)
        );
        assert_eq!(ready.ready_mut().copied(), Some(2));
    }
}
