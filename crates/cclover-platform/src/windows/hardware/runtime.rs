use super::*;

impl CpuRuntime {
    pub(super) fn collect(&mut self) -> std::io::Result<CpuObservation> {
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
    pub(super) fn new(session: Session, board: Option<board::Info>) -> Self {
        Self {
            session,
            collector: superio::Collector::new(board),
        }
    }
}

#[cfg(target_arch = "x86_64")]
impl EcRuntime {
    pub(super) fn supports_temperatures(&self) -> bool {
        match self {
            Self::Acpi { collector, .. } => collector.supports_temperatures(),
            Self::Cros { collector, .. } => collector.supports_temperatures(),
        }
    }

    pub(super) fn supports_fans(&self) -> bool {
        match self {
            Self::Acpi { collector, .. } => collector.supports_fans(),
            Self::Cros { collector, .. } => collector.supports_fans(),
        }
    }

    pub(super) fn collect(&mut self, projection: ec::Projection) -> std::io::Result<EcObservation> {
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

pub(super) fn open_session() -> Result<Session, InitFailure> {
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

pub(super) fn open_module_session(module: &[u8]) -> Result<Session, InitFailure> {
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

pub(super) fn initialize_cpu() -> Result<CpuRuntime, InitFailure> {
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
pub(super) fn initialize_amd_cpu(info: cpu::Info) -> Result<CpuRuntime, InitFailure> {
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
pub(super) fn initialize_amd_cpu(_info: cpu::Info) -> Result<CpuRuntime, InitFailure> {
    Err(InitFailure::Stable(CollectionUnavailable::Unsupported))
}

#[cfg(target_arch = "x86_64")]
pub(super) fn initialize_superio(
    board: Option<board::Info>,
) -> Result<SuperIoRuntime, InitFailure> {
    open_module_session(LPC_IO_MODULE).map(|session| SuperIoRuntime::new(session, board))
}

#[cfg(target_arch = "x86_64")]
pub(super) fn initialize_ec(board: Option<board::Info>) -> Result<EcRuntime, InitFailure> {
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

pub(super) fn project_cpu_sample(
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

pub(super) fn source_transport_failed(error: &std::io::Error) -> bool {
    error.raw_os_error().is_some()
}
