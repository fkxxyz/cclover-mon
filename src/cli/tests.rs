use super::probe::unavailable_reason_name;
use super::*;

fn options(args: &[&str]) -> LaunchRequest {
    parse_launch_options(args.iter().map(|arg| (*arg).to_owned())).unwrap()
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

#[test]
fn terminal_requirement_comes_from_parsed_cli_semantics() {
    let auto = parse_args(std::iter::empty()).unwrap();
    assert!(!auto.requires_terminal());

    let desktop = parse_args(["--desktop".to_owned()].into_iter()).unwrap();
    assert!(!desktop.requires_terminal());

    let tui = parse_args(["--tui".to_owned()].into_iter()).unwrap();
    assert!(tui.requires_terminal());

    let help = parse_args(["help".to_owned()].into_iter()).unwrap();
    assert!(help.requires_terminal());

    let probe = parse_args(["probe".to_owned(), "cpu".to_owned()].into_iter()).unwrap();
    assert!(probe.requires_terminal());
}

#[test]
fn cli_errors_are_reportable_before_execution() {
    assert!(parse_args(["--unknown".to_owned()].into_iter()).is_err());
    assert!(parse_args(["probe".to_owned()].into_iter()).is_err());
    assert!(parse_args(["perf".to_owned()].into_iter()).is_err());
}
