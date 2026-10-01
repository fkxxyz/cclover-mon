use super::*;
pub(super) fn probe_needs_pawnio(kind: ProbeKind) -> bool {
    matches!(kind, ProbeKind::Temperatures | ProbeKind::Fans)
}

pub(super) fn prepare_pawnio_if_needed(needed: bool) {
    #[cfg(target_os = "windows")]
    if needed {
        crate::platform::prepare_machine_capability();
    }
    #[cfg(not(target_os = "windows"))]
    let _ = needed;
}
