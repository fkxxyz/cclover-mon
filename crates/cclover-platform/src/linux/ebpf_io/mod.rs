#[cfg(feature = "ebpf-io")]
mod disk;
#[cfg(feature = "ebpf-io")]
mod network;
#[cfg(feature = "ebpf-io")]
mod runtime;

#[cfg(feature = "ebpf-io")]
mod abi {
    include!(concat!(env!("OUT_DIR"), "/bpf_abi_layout.rs"));
}

#[cfg(feature = "ebpf-io")]
use std::collections::HashSet;
#[cfg(feature = "ebpf-io")]
use std::fmt;

use crate::linux::diagnostics::probe_note;
#[cfg(feature = "ebpf-io")]
use crate::linux::diagnostics::report_issue;
use cclover_core::model::{
    Collection, CollectionUnavailable, ProcessDiskIoCounter, ProcessInstanceId,
    ProcessNetworkIoCounter,
};

#[cfg(feature = "ebpf-io")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FailureKind {
    KernelBtf,
    Privilege,
    AttachPoint,
    MapAccess,
    Disabled,
    Loader,
}

#[cfg(feature = "ebpf-io")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AttributionFailure {
    pub(super) kind: FailureKind,
    detail: String,
}

#[cfg(feature = "ebpf-io")]
impl AttributionFailure {
    fn new(kind: FailureKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }

    fn errno(kind: FailureKind, operation: &str, code: i32) -> Self {
        let errno = code.abs();
        let kind = if matches!(errno, libc::EPERM | libc::EACCES) {
            FailureKind::Privilege
        } else if errno == libc::ENOMEM && kind == FailureKind::KernelBtf {
            FailureKind::MapAccess
        } else {
            kind
        };
        Self::new(
            kind,
            format!("{operation}: {}", std::io::Error::from_raw_os_error(errno)),
        )
    }
}

#[cfg(feature = "ebpf-io")]
impl fmt::Display for AttributionFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let category = match self.kind {
            FailureKind::KernelBtf => "kernel/BTF",
            FailureKind::Privilege => "privilege",
            FailureKind::AttachPoint => "attach-point",
            FailureKind::MapAccess => "map-access",
            FailureKind::Disabled => "disabled",
            FailureKind::Loader => "loader",
        };
        write!(formatter, "{category}: {}", self.detail)
    }
}

pub(super) struct AttributionRows<T> {
    pub(super) rows: Vec<T>,
    #[cfg(feature = "ebpf-io")]
    pub(super) unresolved_native_ids: usize,
}

#[cfg(feature = "ebpf-io")]
fn is_stale_process(
    active_processes: Option<&HashSet<ProcessInstanceId>>,
    process: &ProcessInstanceId,
) -> bool {
    active_processes.is_some_and(|active| !active.contains(process))
}

#[cfg(feature = "ebpf-io")]
pub(super) struct Collector {
    disk: disk::Collector,
    network: network::Collector,
}

#[cfg(feature = "ebpf-io")]
impl Collector {
    pub(super) fn new() -> Self {
        let disabled = std::env::var_os("CCLOVER_MON_DISABLE_EBPF_IO").is_some();
        Self {
            disk: disk::Collector::new(disabled),
            network: network::Collector::new(disabled),
        }
    }

    pub(super) fn collect_disk(
        &mut self,
        active_processes: Option<&HashSet<ProcessInstanceId>>,
        mut notes: Option<&mut Vec<String>>,
    ) -> Collection<AttributionRows<ProcessDiskIoCounter>> {
        match self.disk.collect(active_processes) {
            Ok(result) if result.unresolved_native_ids > 0 => {
                probe_note(&mut notes, || {
                    format!(
                        "{} disk attribution rows skipped: dev_t could not be resolved through sysfs",
                        result.unresolved_native_ids
                    )
                });
                Collection::degraded(result)
            }
            Ok(result) => Collection::available(result),
            Err(error) => {
                let reason = unavailable_reason(error.kind);
                report_issue(&mut notes, || error.to_string());
                Collection::unavailable(reason)
            }
        }
    }

    pub(super) fn collect_network(
        &mut self,
        active_processes: Option<&HashSet<ProcessInstanceId>>,
        mut notes: Option<&mut Vec<String>>,
    ) -> Collection<AttributionRows<ProcessNetworkIoCounter>> {
        match self.network.collect(active_processes) {
            Ok(result) if result.unresolved_native_ids > 0 => {
                probe_note(&mut notes, || {
                    format!(
                        "{} network attribution rows skipped: ifindex could not be resolved in the current network namespace",
                        result.unresolved_native_ids
                    )
                });
                Collection::degraded(result)
            }
            Ok(result) => Collection::available(result),
            Err(error) => {
                let reason = unavailable_reason(error.kind);
                report_issue(&mut notes, || error.to_string());
                Collection::unavailable(reason)
            }
        }
    }
}

#[cfg(not(feature = "ebpf-io"))]
pub(super) struct Collector;

#[cfg(not(feature = "ebpf-io"))]
impl Collector {
    pub(super) fn new() -> Self {
        Self
    }

    pub(super) fn collect_disk(
        &mut self,
        _active_processes: Option<&std::collections::HashSet<ProcessInstanceId>>,
        mut notes: Option<&mut Vec<String>>,
    ) -> Collection<AttributionRows<ProcessDiskIoCounter>> {
        probe_note(&mut notes, || {
            "eBPF disk attribution is disabled in this build".to_owned()
        });
        Collection::unavailable(CollectionUnavailable::Disabled)
    }

    pub(super) fn collect_network(
        &mut self,
        _active_processes: Option<&std::collections::HashSet<ProcessInstanceId>>,
        mut notes: Option<&mut Vec<String>>,
    ) -> Collection<AttributionRows<ProcessNetworkIoCounter>> {
        probe_note(&mut notes, || {
            "eBPF network attribution is disabled in this build".to_owned()
        });
        Collection::unavailable(CollectionUnavailable::Disabled)
    }
}

#[cfg(all(test, feature = "ebpf-io"))]
mod tests {
    use super::*;

    #[test]
    fn stale_attribution_uses_full_process_instance_identity() {
        let current = ProcessInstanceId {
            pid: 42,
            birth_marker: 100,
        };
        let reused_pid = ProcessInstanceId {
            pid: 42,
            birth_marker: 200,
        };
        let active = HashSet::from([current]);

        assert!(!is_stale_process(None, &reused_pid));
        assert!(!is_stale_process(Some(&active), &current));
        assert!(is_stale_process(Some(&active), &reused_pid));
    }
}

#[cfg(feature = "ebpf-io")]
fn unavailable_reason(kind: FailureKind) -> CollectionUnavailable {
    match kind {
        FailureKind::Privilege => CollectionUnavailable::PermissionDenied,
        FailureKind::Disabled => CollectionUnavailable::Disabled,
        FailureKind::KernelBtf | FailureKind::AttachPoint => CollectionUnavailable::Unsupported,
        FailureKind::MapAccess | FailureKind::Loader => CollectionUnavailable::Unavailable,
    }
}
