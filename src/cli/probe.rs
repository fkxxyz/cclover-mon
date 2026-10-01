use super::*;
pub(super) fn probe(kind: ProbeKind, raw: bool) {
    let mut backend = Backend::new();
    let started = Instant::now();
    let report = backend.probe(kind);
    let elapsed = started.elapsed();

    println!("collector: {}", kind.as_str());
    println!("status: {}", probe_status_name(report.status));
    if let CollectionStatus::Unavailable(reason) = report.status {
        println!("reason: {}", unavailable_reason_name(reason));
    }
    println!("elapsed: {:.3} ms", elapsed.as_secs_f64() * 1_000.0);
    for line in report.summary {
        println!("result: {line}");
    }
    if raw {
        for line in report.raw {
            println!("raw: {line}");
        }
    }
    if report.notes.is_empty() {
        println!("diagnostics: none");
    } else {
        for note in report.notes {
            println!("diagnostic: {note}");
        }
    }
}

pub(super) fn probe_status_name(status: CollectionStatus) -> &'static str {
    match status {
        CollectionStatus::Available => "ok",
        CollectionStatus::Degraded => "degraded",
        CollectionStatus::Unavailable(_) => "unavailable",
    }
}

pub(super) fn unavailable_reason_name(reason: CollectionUnavailable) -> &'static str {
    match reason {
        CollectionUnavailable::Unsupported => "unsupported",
        CollectionUnavailable::Disabled => "disabled",
        CollectionUnavailable::PermissionDenied => "permission-denied",
        CollectionUnavailable::Unavailable => "unavailable",
        CollectionUnavailable::InvalidData => "invalid-data",
    }
}
