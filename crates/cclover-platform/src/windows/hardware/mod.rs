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

use cclover_core::model::{Collection, CollectionUnavailable, FanSnapshot, TemperatureSnapshot};

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

mod collector;
mod merge;
mod runtime;
mod source;

use merge::merge_fan_sources;
pub(super) use merge::merge_temperature_sources;
#[cfg(target_arch = "x86_64")]
use merge::projected_collection;
use runtime::{initialize_cpu, project_cpu_sample};
#[cfg(target_arch = "x86_64")]
use runtime::{initialize_ec, initialize_superio, source_transport_failed};

#[cfg(test)]
mod tests;
