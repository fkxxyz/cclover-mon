use super::*;

impl Collector {
    pub(in crate::windows) fn new() -> Self {
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

    pub(in crate::windows) fn collect(&mut self, mut notes: Option<&mut Vec<String>>) -> Batch {
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

    pub(in crate::windows) fn collect_temperatures(
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

    pub(in crate::windows) fn collect_fans(
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
