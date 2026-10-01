#[cfg(test)]
mod tests {
    use super::*;
    use cclover_core::model::TemperatureId;

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
