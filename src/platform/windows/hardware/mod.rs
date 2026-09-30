#[cfg(target_arch = "x86_64")]
mod amd;
#[cfg(target_arch = "x86_64")]
mod board;
mod coretemp;
mod cpu;
#[cfg(target_arch = "x86_64")]
mod cros_ec;
#[cfg(target_arch = "x86_64")]
mod ec;
mod intel;
#[cfg(target_arch = "x86_64")]
mod k10temp;
#[cfg(target_arch = "x86_64")]
mod k8temp;
#[cfg(target_arch = "x86_64")]
mod superio;
#[cfg(target_arch = "x86_64")]
mod sync;

use std::time::{Duration, Instant};

use crate::core::model::{Collection, CollectionUnavailable, FanSnapshot, TemperatureSnapshot};

use super::diagnostics::{report_issue, unavailable_from_io};
use super::pawnio::{OpenError, Session};
use super::provision::{self, MachineStatus};

const INTEL_MSR_MODULE: &[u8] = include_bytes!(env!("CCLOVER_PAWNIO_INTEL_MSR_BIN"));
#[cfg(target_arch = "x86_64")]
const LPC_IO_MODULE: &[u8] = include_bytes!(env!("CCLOVER_PAWNIO_LPC_IO_BIN"));
#[cfg(target_arch = "x86_64")]
const LPC_ACPI_EC_MODULE: &[u8] = include_bytes!(env!("CCLOVER_PAWNIO_LPC_ACPI_EC_BIN"));
#[cfg(target_arch = "x86_64")]
const LPC_CROS_EC_MODULE: &[u8] = include_bytes!(env!("CCLOVER_PAWNIO_LPC_CROS_EC_BIN"));
const RETRY_INTERVAL: Duration = Duration::from_secs(30);

pub(super) struct Collector {
    cpu: Source<CpuRuntime>,
    #[cfg(target_arch = "x86_64")]
    board: Option<board::Info>,
    #[cfg(target_arch = "x86_64")]
    superio: Source<SuperIoRuntime>,
    #[cfg(target_arch = "x86_64")]
    ec: Source<EcRuntime>,
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

struct CpuRuntime {
    session: Session,
    collector: CpuCollector,
}

enum CpuCollector {
    Intel(intel::Collector),
    #[cfg(target_arch = "x86_64")]
    Amd(amd::Collector),
}

struct CpuObservation {
    temperatures: Vec<CpuTemperature>,
    degraded: bool,
    diagnostic: CpuDiagnostic,
}

struct CpuSampleDecision {
    collection: Collection<Vec<TemperatureSnapshot>>,
    retry_reason: Option<CollectionUnavailable>,
}

#[derive(Debug)]
enum CpuDiagnostic {
    Intel {
        core_count: usize,
        affinity_failures: usize,
        invalid_core_dts: usize,
    },
    #[cfg(target_arch = "x86_64")]
    Amd,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CpuTemperatureVisibility {
    Primary,
    Detail,
}

pub(super) struct CpuTemperature {
    pub snapshot: TemperatureSnapshot,
    pub visibility: CpuTemperatureVisibility,
}

#[derive(Clone, Copy)]
enum CpuTemperatureProjection {
    Product,
    Diagnostic,
}

#[cfg(target_arch = "x86_64")]
struct SuperIoRuntime {
    session: Session,
    collector: superio::Collector,
}

#[cfg(target_arch = "x86_64")]
enum EcRuntime {
    Acpi {
        session: Session,
        collector: ec::Collector,
    },
    Cros {
        session: Session,
        collector: cros_ec::Collector,
    },
}

#[cfg(target_arch = "x86_64")]
struct EcObservation {
    temperatures: Vec<TemperatureSnapshot>,
    fans: Vec<FanSnapshot>,
    temperature_degraded: bool,
    fan_degraded: bool,
}

#[derive(Debug)]
enum InitFailure {
    Retry(CollectionUnavailable),
    Stable(CollectionUnavailable),
}

#[derive(Clone, Copy)]
enum SuperIoProjection {
    All,
    Temperatures,
    Fans,
}

impl Collector {
    pub(super) fn new() -> Self {
        #[cfg(target_arch = "x86_64")]
        let board = board::detect().ok();
        Self {
            cpu: Source::new(),
            #[cfg(target_arch = "x86_64")]
            board,
            #[cfg(target_arch = "x86_64")]
            superio: Source::new(),
            #[cfg(target_arch = "x86_64")]
            ec: Source::new(),
        }
    }

    pub(super) fn collect(&mut self, mut notes: Option<&mut Vec<String>>) -> Batch {
        let now = Instant::now();
        let cpu_temperatures = self.collect_cpu_temperatures_at(
            now,
            notes.as_deref_mut(),
            CpuTemperatureProjection::Product,
        );
        let superio = self.collect_superio_at(now, notes.as_deref_mut(), SuperIoProjection::All);
        let ec = self.collect_ec_at(now, notes, SuperIoProjection::All);
        Batch {
            temperatures: merge_temperature_sources([
                cpu_temperatures,
                superio.temperatures,
                ec.temperatures,
            ]),
            fans: merge_fan_sources([superio.fans, ec.fans]),
        }
    }

    pub(super) fn collect_temperatures(
        &mut self,
        mut notes: Option<&mut Vec<String>>,
    ) -> Collection<Vec<TemperatureSnapshot>> {
        let now = Instant::now();
        let cpu_temperatures = self.collect_cpu_temperatures_at(
            now,
            notes.as_deref_mut(),
            CpuTemperatureProjection::Diagnostic,
        );
        let superio =
            self.collect_superio_at(now, notes.as_deref_mut(), SuperIoProjection::Temperatures);
        let ec = self.collect_ec_at(now, notes, SuperIoProjection::Temperatures);
        merge_temperature_sources([cpu_temperatures, superio.temperatures, ec.temperatures])
    }

    pub(super) fn collect_fans(
        &mut self,
        mut notes: Option<&mut Vec<String>>,
    ) -> Collection<Vec<FanSnapshot>> {
        let now = Instant::now();
        let superio = self.collect_superio_at(now, notes.as_deref_mut(), SuperIoProjection::Fans);
        let ec = self.collect_ec_at(now, notes, SuperIoProjection::Fans);
        merge_fan_sources([superio.fans, ec.fans])
    }

    fn collect_cpu_temperatures_at(
        &mut self,
        now: Instant,
        mut notes: Option<&mut Vec<String>>,
        projection: CpuTemperatureProjection,
    ) -> Collection<Vec<TemperatureSnapshot>> {
        if let Err(reason) = self.cpu.ensure_ready(now, initialize_cpu) {
            report_issue(&mut notes, || {
                format!("CPU temperature source unavailable: {reason:?}")
            });
            return Collection::unavailable(reason);
        }

        let result = self
            .cpu
            .ready_mut()
            .expect("CPU telemetry source is ready after ensure_ready")
            .collect();
        match &result {
            Ok(observation) => {
                let count = observation.temperatures.len();
                let degraded = observation.degraded;
                report_issue(&mut notes, || {
                    let detail = match &observation.diagnostic {
                        CpuDiagnostic::Intel {
                            core_count,
                            affinity_failures,
                            invalid_core_dts,
                        } => format!(
                            "kind=intel cores={core_count} affinity_failures={affinity_failures} invalid_core_dts={invalid_core_dts}"
                        ),
                        #[cfg(target_arch = "x86_64")]
                        CpuDiagnostic::Amd => "kind=amd".to_owned(),
                    };
                    format!("CPU temperature source: sensors={count} degraded={degraded} {detail}")
                });
            }
            Err(error) => {
                report_issue(&mut notes, || {
                    format!("PawnIO CPU temperature sampling failed: {error}")
                });
            }
        }
        let decision = project_cpu_sample(result, projection);
        if let Some(reason) = decision.retry_reason {
            self.cpu.retry(now, reason);
        }
        decision.collection
    }

    #[cfg(target_arch = "x86_64")]
    fn collect_superio_at(
        &mut self,
        now: Instant,
        notes: Option<&mut Vec<String>>,
        projection: SuperIoProjection,
    ) -> Batch {
        let board = self.board.clone();
        if let Err(reason) = self.superio.ensure_ready(now, || initialize_superio(board)) {
            let mut notes = notes;
            report_issue(&mut notes, || {
                format!("Super-I/O source unavailable: {reason:?}")
            });
            return Batch::for_superio_unavailable(reason, projection);
        }

        let runtime = self
            .superio
            .ready_mut()
            .expect("Super-I/O source is ready after ensure_ready");
        let result = match projection {
            SuperIoProjection::All => runtime.collector.collect(&runtime.session, notes),
            SuperIoProjection::Temperatures => runtime
                .collector
                .collect_temperatures(&runtime.session, notes),
            SuperIoProjection::Fans => runtime.collector.collect_fans(&runtime.session, notes),
        };
        match result {
            superio::CollectOutcome::Observation(observation) => Batch {
                temperatures: observation.temperatures,
                fans: observation.fans,
            },
            superio::CollectOutcome::SourceFailed(error) => {
                let reason = if error.kind() == std::io::ErrorKind::PermissionDenied {
                    CollectionUnavailable::PermissionDenied
                } else {
                    CollectionUnavailable::Unavailable
                };
                self.superio.retry(now, reason);
                Batch::for_superio_unavailable(reason, projection)
            }
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn collect_superio_at(
        &mut self,
        _now: Instant,
        _notes: Option<&mut Vec<String>>,
        projection: SuperIoProjection,
    ) -> Batch {
        Batch::for_superio_unavailable(CollectionUnavailable::Unsupported, projection)
    }

    #[cfg(target_arch = "x86_64")]
    fn collect_ec_at(
        &mut self,
        now: Instant,
        mut notes: Option<&mut Vec<String>>,
        projection: SuperIoProjection,
    ) -> Batch {
        let board = self.board.clone();
        if let Err(reason) = self.ec.ensure_ready(now, || initialize_ec(board)) {
            report_issue(&mut notes, || {
                let identity = self
                    .board
                    .as_ref()
                    .map(|board| format!("{} / {}", board.manufacturer, board.product))
                    .unwrap_or_else(|| "unknown board".to_owned());
                format!("ACPI EC source unavailable: {reason:?}; board={identity}")
            });
            return Batch::for_superio_unavailable(reason, projection);
        }

        let runtime = self
            .ec
            .ready_mut()
            .expect("EC source is ready after ensure_ready");
        let want_temperatures = matches!(
            projection,
            SuperIoProjection::All | SuperIoProjection::Temperatures
        );
        let want_fans = matches!(projection, SuperIoProjection::All | SuperIoProjection::Fans);
        let supports_temperatures = runtime.supports_temperatures();
        let supports_fans = runtime.supports_fans();
        let ec_projection = match projection {
            SuperIoProjection::All => ec::Projection {
                temperatures: true,
                fans: true,
            },
            SuperIoProjection::Temperatures => ec::Projection {
                temperatures: true,
                fans: false,
            },
            SuperIoProjection::Fans => ec::Projection {
                temperatures: false,
                fans: true,
            },
        };
        let result = runtime.collect(ec_projection);
        match result {
            Ok(observation) => Batch {
                temperatures: projected_collection(
                    want_temperatures && supports_temperatures,
                    observation.temperature_degraded,
                    observation.temperatures,
                ),
                fans: projected_collection(
                    want_fans && supports_fans,
                    observation.fan_degraded,
                    observation.fans,
                ),
            },
            Err(error) => {
                report_issue(&mut notes, || {
                    format!("PawnIO embedded-controller sampling failed: {error}")
                });
                let reason = unavailable_from_io(&error);
                if source_transport_failed(&error) {
                    self.ec.retry(now, reason);
                }
                Batch::for_superio_unavailable(reason, projection)
            }
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn collect_ec_at(
        &mut self,
        _now: Instant,
        _notes: Option<&mut Vec<String>>,
        projection: SuperIoProjection,
    ) -> Batch {
        Batch::for_superio_unavailable(CollectionUnavailable::Unsupported, projection)
    }
}

impl Batch {
    fn for_superio_unavailable(
        reason: CollectionUnavailable,
        projection: SuperIoProjection,
    ) -> Self {
        Self {
            temperatures: if matches!(
                projection,
                SuperIoProjection::All | SuperIoProjection::Temperatures
            ) {
                Collection::unavailable(reason)
            } else {
                Collection::unavailable(CollectionUnavailable::Unsupported)
            },
            fans: if matches!(projection, SuperIoProjection::All | SuperIoProjection::Fans) {
                Collection::unavailable(reason)
            } else {
                Collection::unavailable(CollectionUnavailable::Unsupported)
            },
        }
    }
}

#[cfg(target_arch = "x86_64")]
fn projected_collection<T>(requested: bool, degraded: bool, values: Vec<T>) -> Collection<Vec<T>> {
    if !requested {
        Collection::unavailable(CollectionUnavailable::Unsupported)
    } else if degraded {
        Collection::degraded(values)
    } else {
        Collection::available(values)
    }
}

pub(super) fn merge_temperature_sources(
    sources: impl IntoIterator<Item = Collection<Vec<TemperatureSnapshot>>>,
) -> Collection<Vec<TemperatureSnapshot>> {
    merge_sources(sources)
}

fn merge_fan_sources(
    sources: impl IntoIterator<Item = Collection<Vec<FanSnapshot>>>,
) -> Collection<Vec<FanSnapshot>> {
    merge_sources(sources)
}

fn merge_sources<T>(sources: impl IntoIterator<Item = Collection<Vec<T>>>) -> Collection<Vec<T>> {
    let mut values = Vec::new();
    let mut observable = false;
    let mut degraded = false;
    let mut unavailable_reason = CollectionUnavailable::Unsupported;

    for source in sources {
        match source {
            Collection::Available(mut source_values) => {
                observable = true;
                values.append(&mut source_values);
            }
            Collection::Degraded(mut source_values) => {
                observable = true;
                degraded = true;
                values.append(&mut source_values);
            }
            Collection::Unavailable(CollectionUnavailable::Unsupported) => {}
            Collection::Unavailable(reason) => {
                degraded = true;
                unavailable_reason = reason;
            }
        }
    }

    if observable {
        if degraded {
            Collection::degraded(values)
        } else {
            Collection::available(values)
        }
    } else {
        Collection::unavailable(unavailable_reason)
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

impl CpuRuntime {
    fn collect(&mut self) -> std::io::Result<CpuObservation> {
        match &mut self.collector {
            CpuCollector::Intel(collector) => {
                collector
                    .collect(&self.session)
                    .map(|observation| CpuObservation {
                        temperatures: observation.temperatures,
                        degraded: observation.degraded,
                        diagnostic: CpuDiagnostic::Intel {
                            core_count: observation.core_count,
                            affinity_failures: observation.affinity_failures,
                            invalid_core_dts: observation.invalid_core_dts,
                        },
                    })
            }
            #[cfg(target_arch = "x86_64")]
            CpuCollector::Amd(collector) => {
                collector
                    .collect(&self.session)
                    .map(|observation| CpuObservation {
                        temperatures: observation.temperatures,
                        degraded: observation.degraded,
                        diagnostic: CpuDiagnostic::Amd,
                    })
            }
        }
    }
}

#[cfg(target_arch = "x86_64")]
impl SuperIoRuntime {
    fn new(session: Session, board: Option<board::Info>) -> Self {
        Self {
            session,
            collector: superio::Collector::new(board),
        }
    }
}

#[cfg(target_arch = "x86_64")]
impl EcRuntime {
    fn supports_temperatures(&self) -> bool {
        match self {
            Self::Acpi { collector, .. } => collector.supports_temperatures(),
            Self::Cros { collector, .. } => collector.supports_temperatures(),
        }
    }

    fn supports_fans(&self) -> bool {
        match self {
            Self::Acpi { collector, .. } => collector.supports_fans(),
            Self::Cros { collector, .. } => collector.supports_fans(),
        }
    }

    fn collect(&mut self, projection: ec::Projection) -> std::io::Result<EcObservation> {
        match self {
            Self::Acpi { session, collector } => {
                collector
                    .collect(session, projection)
                    .map(|observation| EcObservation {
                        temperatures: observation.temperatures,
                        fans: observation.fans,
                        temperature_degraded: observation.temperature_degraded,
                        fan_degraded: observation.fan_degraded,
                    })
            }
            Self::Cros { session, collector } => {
                collector
                    .collect(session, projection)
                    .map(|observation| EcObservation {
                        temperatures: observation.temperatures,
                        fans: observation.fans,
                        temperature_degraded: observation.temperature_degraded,
                        fan_degraded: observation.fan_degraded,
                    })
            }
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

fn initialize_cpu() -> Result<CpuRuntime, InitFailure> {
    let info = cpu::detect();
    match info.vendor {
        cpu::Vendor::Intel => {
            let collector = intel::Collector::new(info)
                .map_err(|_| InitFailure::Retry(CollectionUnavailable::Unavailable))?;
            let session = open_module_session(INTEL_MSR_MODULE)?;
            Ok(CpuRuntime {
                session,
                collector: CpuCollector::Intel(collector),
            })
        }
        cpu::Vendor::Amd => initialize_amd_cpu(info),
        cpu::Vendor::Other => Err(InitFailure::Stable(CollectionUnavailable::Unsupported)),
    }
}

#[cfg(target_arch = "x86_64")]
fn initialize_amd_cpu(info: cpu::Info) -> Result<CpuRuntime, InitFailure> {
    let collector = amd::Collector::new(info)
        .map_err(|_| InitFailure::Retry(CollectionUnavailable::Unavailable))?
        .ok_or(InitFailure::Stable(CollectionUnavailable::Unsupported))?;
    let session = open_module_session(collector.module())?;
    Ok(CpuRuntime {
        session,
        collector: CpuCollector::Amd(collector),
    })
}

#[cfg(not(target_arch = "x86_64"))]
fn initialize_amd_cpu(_info: cpu::Info) -> Result<CpuRuntime, InitFailure> {
    Err(InitFailure::Stable(CollectionUnavailable::Unsupported))
}

#[cfg(target_arch = "x86_64")]
fn initialize_superio(board: Option<board::Info>) -> Result<SuperIoRuntime, InitFailure> {
    open_module_session(LPC_IO_MODULE).map(|session| SuperIoRuntime::new(session, board))
}

#[cfg(target_arch = "x86_64")]
fn initialize_ec(board: Option<board::Info>) -> Result<EcRuntime, InitFailure> {
    let board = board.ok_or(InitFailure::Stable(CollectionUnavailable::Unsupported))?;
    if let Some(collector) = ec::Collector::new(&board) {
        let session = open_module_session(LPC_ACPI_EC_MODULE)?;
        return Ok(EcRuntime::Acpi { session, collector });
    }

    let board_key = board.product_key();
    if cros_ec::is_supported_board(&board_key) {
        let session = open_module_session(LPC_CROS_EC_MODULE)?;
        let collector = cros_ec::Collector::discover(&session, board_key)
            .map_err(|error| InitFailure::Retry(unavailable_from_io(&error)))?;
        return Ok(EcRuntime::Cros { session, collector });
    }

    Err(InitFailure::Stable(CollectionUnavailable::Unsupported))
}

fn project_cpu_sample(
    result: std::io::Result<CpuObservation>,
    projection: CpuTemperatureProjection,
) -> CpuSampleDecision {
    match result {
        Ok(observation) => {
            let temperatures = observation
                .temperatures
                .into_iter()
                .filter(|temperature| {
                    matches!(projection, CpuTemperatureProjection::Diagnostic)
                        || temperature.visibility == CpuTemperatureVisibility::Primary
                })
                .map(|temperature| temperature.snapshot)
                .collect();
            let collection = if observation.degraded {
                Collection::degraded(temperatures)
            } else {
                Collection::available(temperatures)
            };
            CpuSampleDecision {
                collection,
                retry_reason: None,
            }
        }
        Err(error) => {
            let reason = unavailable_from_io(&error);
            CpuSampleDecision {
                collection: Collection::unavailable(reason),
                retry_reason: source_transport_failed(&error).then_some(reason),
            }
        }
    }
}

fn source_transport_failed(error: &std::io::Error) -> bool {
    error.raw_os_error().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::model::TemperatureId;

    fn cpu_observation(degraded: bool) -> CpuObservation {
        CpuObservation {
            temperatures: vec![
                CpuTemperature {
                    snapshot: TemperatureSnapshot {
                        id: TemperatureId::from_opaque_key("cpu-primary"),
                        name: "package".to_owned(),
                        celsius: 52.0,
                    },
                    visibility: CpuTemperatureVisibility::Primary,
                },
                CpuTemperature {
                    snapshot: TemperatureSnapshot {
                        id: TemperatureId::from_opaque_key("cpu-detail"),
                        name: "core".to_owned(),
                        celsius: 48.0,
                    },
                    visibility: CpuTemperatureVisibility::Detail,
                },
            ],
            degraded,
            diagnostic: CpuDiagnostic::Intel {
                core_count: 1,
                affinity_failures: 0,
                invalid_core_dts: 0,
            },
        }
    }

    #[test]
    fn cpu_product_and_diagnostic_projections_use_typed_visibility() {
        let product = project_cpu_sample(
            Ok(cpu_observation(false)),
            CpuTemperatureProjection::Product,
        );
        let Collection::Available(product) = product.collection else {
            panic!("healthy CPU sample must remain available");
        };
        assert_eq!(product.len(), 1);
        assert_eq!(product[0].id.as_opaque_key(), "cpu-primary");

        let diagnostic = project_cpu_sample(
            Ok(cpu_observation(false)),
            CpuTemperatureProjection::Diagnostic,
        );
        let Collection::Available(diagnostic) = diagnostic.collection else {
            panic!("healthy diagnostic CPU sample must remain available");
        };
        assert_eq!(diagnostic.len(), 2);
        assert_eq!(diagnostic[0].id.as_opaque_key(), "cpu-primary");
        assert_eq!(diagnostic[1].id.as_opaque_key(), "cpu-detail");
    }

    #[test]
    fn cpu_partial_success_projects_as_degraded_without_retry() {
        let decision = project_cpu_sample(
            Ok(cpu_observation(true)),
            CpuTemperatureProjection::Diagnostic,
        );
        assert!(matches!(decision.collection, Collection::Degraded(values) if values.len() == 2));
        assert_eq!(decision.retry_reason, None);
    }

    #[test]
    fn cpu_transport_failure_becomes_unavailable_and_requests_retry() {
        let error = std::io::Error::from_raw_os_error(5);
        let expected = unavailable_from_io(&error);
        let decision = project_cpu_sample(Err(error), CpuTemperatureProjection::Product);
        assert!(matches!(
            decision.collection,
            Collection::Unavailable(reason) if reason == expected
        ));
        assert_eq!(decision.retry_reason, Some(expected));
    }

    #[test]
    fn cpu_nontransport_invalid_data_is_unavailable_without_session_retry() {
        let error = std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid channel data");
        let decision = project_cpu_sample(Err(error), CpuTemperatureProjection::Product);
        assert!(matches!(
            decision.collection,
            Collection::Unavailable(CollectionUnavailable::InvalidData)
        ));
        assert_eq!(decision.retry_reason, None);
    }

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
        assert_eq!(failed.ready_mut(), None);
        assert_eq!(ready.ready_mut().copied(), Some(2));
    }

    #[test]
    fn merging_sources_preserves_values_and_degradation() {
        let cpu = TemperatureSnapshot {
            id: TemperatureId::from_opaque_key("cpu"),
            name: "CPU".to_owned(),
            celsius: 50.0,
        };
        let board = TemperatureSnapshot {
            id: TemperatureId::from_opaque_key("board"),
            name: "Board".to_owned(),
            celsius: 40.0,
        };
        let merged = merge_temperature_sources([
            Collection::available(vec![cpu]),
            Collection::degraded(vec![board]),
        ]);
        assert!(matches!(merged, Collection::Degraded(values) if values.len() == 2));
    }

    #[test]
    fn unsupported_optional_source_does_not_degrade_working_temperature_source() {
        let cpu = TemperatureSnapshot {
            id: TemperatureId::from_opaque_key("cpu"),
            name: "CPU".to_owned(),
            celsius: 50.0,
        };
        let merged = merge_temperature_sources([
            Collection::available(vec![cpu]),
            Collection::unavailable(CollectionUnavailable::Unsupported),
        ]);
        assert!(matches!(merged, Collection::Available(values) if values.len() == 1));
    }
}
