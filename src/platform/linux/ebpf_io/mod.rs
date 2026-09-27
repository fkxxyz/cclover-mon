mod disk;
mod network;
mod runtime;

mod abi {
    include!(concat!(env!("OUT_DIR"), "/bpf_abi_layout.rs"));
}

use std::fmt;

use crate::core::model::{ProcessDiskIoCounter, ProcessNetworkIoCounter};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FailureKind {
    KernelBtf,
    Privilege,
    AttachPoint,
    MapAccess,
    Disabled,
    Loader,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AttributionFailure {
    pub(super) kind: FailureKind,
    detail: String,
}

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
    pub(super) unresolved_native_ids: usize,
}

pub(super) struct Collector {
    disk: disk::Collector,
    network: network::Collector,
}

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
    ) -> Result<AttributionRows<ProcessDiskIoCounter>, AttributionFailure> {
        self.disk.collect()
    }

    pub(super) fn collect_network(
        &mut self,
    ) -> Result<AttributionRows<ProcessNetworkIoCounter>, AttributionFailure> {
        self.network.collect()
    }
}
