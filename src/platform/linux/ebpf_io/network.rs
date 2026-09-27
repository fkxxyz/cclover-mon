use std::ffi::CStr;
use std::mem::offset_of;

use crate::core::model::{ProcessInstanceId, ProcessNetworkIoCounter};

use super::abi;
use super::runtime::{LoadedObject, read_map};
use super::{AttributionFailure, AttributionRows, FailureKind};
use crate::platform::linux::native;
use crate::platform::linux::process::birth_marker_from_start_boottime_ns;

const OBJECT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/network_attribution.bpf.o"));
const MAP: &CStr = c"network_bytes";

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Key {
    tgid: u32,
    ifindex: u32,
    direction: u8,
    pad: [u8; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CounterValue {
    process_start_time: u64,
    bytes: u64,
}

const _: () = {
    assert!(size_of::<Key>() == abi::network_key::SIZE);
    assert!(align_of::<Key>() == abi::network_key::ALIGN);
    assert!(offset_of!(Key, tgid) == abi::network_key::TGID_OFFSET);
    assert!(offset_of!(Key, ifindex) == abi::network_key::IFINDEX_OFFSET);
    assert!(offset_of!(Key, direction) == abi::network_key::DIRECTION_OFFSET);
    assert!(offset_of!(Key, pad) == abi::network_key::PAD_OFFSET);

    assert!(size_of::<CounterValue>() == abi::network_counter_value::SIZE);
    assert!(align_of::<CounterValue>() == abi::network_counter_value::ALIGN);
    assert!(
        offset_of!(CounterValue, process_start_time)
            == abi::network_counter_value::PROCESS_START_TIME_OFFSET
    );
    assert!(offset_of!(CounterValue, bytes) == abi::network_counter_value::BYTES_OFFSET);
};

pub(super) struct Collector {
    disabled: bool,
    object: Option<Result<LoadedObject, AttributionFailure>>,
}

impl Collector {
    pub(super) fn new(disabled: bool) -> Self {
        Self {
            disabled,
            object: None,
        }
    }

    pub(super) fn collect(
        &mut self,
    ) -> Result<AttributionRows<ProcessNetworkIoCounter>, AttributionFailure> {
        let object = self.object()?;
        let mut rows = Vec::new();
        let mut unresolved_native_ids = 0;
        for (key, value) in read_map::<Key, CounterValue>(object.map_fd(MAP)?)? {
            let Some(interface) = native::interface_name(key.ifindex) else {
                unresolved_native_ids += 1;
                continue;
            };
            rows.push(ProcessNetworkIoCounter {
                process: ProcessInstanceId {
                    pid: key.tgid,
                    birth_marker: birth_marker_from_start_boottime_ns(value.process_start_time),
                },
                interface,
                rx_bytes: if key.direction == 0 { value.bytes } else { 0 },
                tx_bytes: if key.direction == 1 { value.bytes } else { 0 },
            });
        }
        merge_rows(&mut rows);
        Ok(AttributionRows {
            rows,
            unresolved_native_ids,
        })
    }

    fn object(&mut self) -> Result<&LoadedObject, AttributionFailure> {
        if self.object.is_none() {
            self.object = Some(if self.disabled {
                Err(AttributionFailure::new(
                    FailureKind::Disabled,
                    "CCLOVER_MON_DISABLE_EBPF_IO is set",
                ))
            } else {
                LoadedObject::load(OBJECT)
            });
        }
        self.object
            .as_ref()
            .expect("network attribution initialized")
            .as_ref()
            .map_err(Clone::clone)
    }
}

fn merge_rows(rows: &mut Vec<ProcessNetworkIoCounter>) {
    rows.sort_by(|a, b| (a.process, &a.interface).cmp(&(b.process, &b.interface)));
    let mut merged: Vec<ProcessNetworkIoCounter> = Vec::with_capacity(rows.len());
    for row in rows.drain(..) {
        if let Some(last) = merged.last_mut()
            && last.process == row.process
            && last.interface == row.interface
        {
            last.rx_bytes = last.rx_bytes.saturating_add(row.rx_bytes);
            last.tx_bytes = last.tx_bytes.saturating_add(row.tx_bytes);
        } else {
            merged.push(row);
        }
    }
    *rows = merged;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_directions_without_crossing_pid_or_interface() {
        let mut rows = vec![
            ProcessNetworkIoCounter {
                process: ProcessInstanceId {
                    pid: 7,
                    birth_marker: 1,
                },
                interface: "lo".into(),
                rx_bytes: 11,
                tx_bytes: 0,
            },
            ProcessNetworkIoCounter {
                process: ProcessInstanceId {
                    pid: 7,
                    birth_marker: 1,
                },
                interface: "lo".into(),
                rx_bytes: 0,
                tx_bytes: 22,
            },
            ProcessNetworkIoCounter {
                process: ProcessInstanceId {
                    pid: 7,
                    birth_marker: 1,
                },
                interface: "eth0".into(),
                rx_bytes: 33,
                tx_bytes: 0,
            },
        ];
        merge_rows(&mut rows);
        assert_eq!(rows.len(), 2);
        assert!(
            rows.iter()
                .any(|row| row.interface == "lo" && row.rx_bytes == 11 && row.tx_bytes == 22)
        );
    }

    #[test]
    fn unresolved_native_ids_are_unavailable_not_fabricated_names() {
        assert_eq!(native::interface_name(u32::MAX), None);
    }
}
