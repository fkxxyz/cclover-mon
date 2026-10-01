use super::probe::unavailable_reason_name;
use super::*;

fn options(args: &[&str]) -> LaunchRequest {
    parse_launch_options(args.iter().map(|arg| (*arg).to_owned()))
}

#[test]
fn no_frontend_flag_requests_auto_selection() {
    assert_eq!(options(&[]), LaunchRequest::Auto);
}

#[cfg(feature = "http")]
#[test]
fn explicit_frontends_are_composable_and_disable_implicit_desktop() {
    assert_eq!(
        options(&["--tui", "--http"]),
        LaunchRequest::Explicit {
            desktop: false,
            tui: true,
            http: Some(HttpConfig::default()),
        }
    );
    assert_eq!(
        options(&["--desktop", "--tui", "--http"]),
        LaunchRequest::Explicit {
            desktop: true,
            tui: true,
            http: Some(HttpConfig::default()),
        }
    );
}

#[test]
fn pawnio_probe_requirement_covers_hardware_telemetry() {
    assert!(probe_needs_pawnio(ProbeKind::Temperatures));
    assert!(probe_needs_pawnio(ProbeKind::Fans));
    assert!(!probe_needs_pawnio(ProbeKind::Cpu));
}

#[test]
fn probe_unavailability_reasons_have_stable_cli_names() {
    assert_eq!(
        unavailable_reason_name(CollectionUnavailable::PermissionDenied),
        "permission-denied"
    );
    assert_eq!(
        unavailable_reason_name(CollectionUnavailable::Unsupported),
        "unsupported"
    );
    assert_eq!(
        unavailable_reason_name(CollectionUnavailable::InvalidData),
        "invalid-data"
    );
}

#[cfg(not(feature = "http"))]
#[test]
fn minimal_build_composes_native_frontends_without_http() {
    assert_eq!(
        options(&["--desktop", "--tui"]),
        LaunchRequest::Explicit {
            desktop: true,
            tui: true,
            http: None,
        }
    );
}
