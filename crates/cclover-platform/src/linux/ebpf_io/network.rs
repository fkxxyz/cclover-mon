use std::collections::{HashMap, HashSet};
use std::ffi::CStr;
use std::mem::offset_of;

use cclover_core::model::{ProcessInstanceId, ProcessNetworkIoCounter};

use super::abi;
use super::runtime::{LoadedObject, delete_map_keys, read_map};
use super::{AttributionFailure, AttributionRows, FailureKind, is_stale_process};
use crate::linux::native;
use crate::linux::network as network_metric;
use crate::linux::process::birth_marker_from_start_boottime_ns;

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
        active_processes: Option<&HashSet<ProcessInstanceId>>,
    ) -> Result<AttributionRows<ProcessNetworkIoCounter>, AttributionFailure> {
        let object = self.object()?;
        let map_fd = object.map_fd(MAP)?;
        let map_read = read_map::<Key, CounterValue>(map_fd)?;
        let batch_fallback = map_read.batch_fallback;
        let mut rows = Vec::new();
        let mut stale_keys = Vec::new();
        let mut unresolved_native_ids = 0;
        let mut identities = HashMap::new();
        for (key, value) in map_read.rows {
            let process = ProcessInstanceId {
                pid: key.tgid,
                birth_marker: birth_marker_from_start_boottime_ns(value.process_start_time),
            };
            if is_stale_process(active_processes, &process) {
                stale_keys.push(key);
                continue;
            }
            let Some((interface, network_id)) =
                resolve_identity(&mut identities, key.ifindex, |ifindex| {
                    let interface = native::interface_name(ifindex)?;
                    let network_id = network_metric::id_for_interface(&interface, ifindex)?;
                    Some((interface, network_id))
                })
            else {
                unresolved_native_ids += 1;
                continue;
            };
            rows.push(ProcessNetworkIoCounter {
                process,
                network_id: network_id.clone(),
                interface: interface.clone(),
                rx_bytes: if key.direction == 0 { value.bytes } else { 0 },
                tx_bytes: if key.direction == 1 { value.bytes } else { 0 },
            });
        }
        delete_map_keys(map_fd, &stale_keys)?;
        merge_rows(&mut rows);
        Ok(AttributionRows {
            rows,
            unresolved_native_ids,
            batch_fallback,
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

fn resolve_identity(
    identities: &mut HashMap<u32, Option<(String, cclover_core::model::NetworkId)>>,
    ifindex: u32,
    resolve: impl FnOnce(u32) -> Option<(String, cclover_core::model::NetworkId)>,
) -> Option<&(String, cclover_core::model::NetworkId)> {
    identities
        .entry(ifindex)
        .or_insert_with(|| resolve(ifindex))
        .as_ref()
}

fn merge_rows(rows: &mut Vec<ProcessNetworkIoCounter>) {
    rows.sort_by(|a, b| (a.process, &a.network_id).cmp(&(b.process, &b.network_id)));
    let mut merged: Vec<ProcessNetworkIoCounter> = Vec::with_capacity(rows.len());
    for row in rows.drain(..) {
        if let Some(last) = merged.last_mut()
            && last.process == row.process
            && last.network_id == row.network_id
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
    use cclover_core::model::NetworkId;

    fn network_id(key: &str) -> NetworkId {
        NetworkId::from_opaque_key(key)
    }

    const BPF_SOURCE: &str = include_str!("../../../native/linux/bpf/network_attribution.bpf.c");

    fn bpf_function(name: &str) -> &str {
        let start = BPF_SOURCE
            .find(&format!("void {name}("))
            .unwrap_or_else(|| panic!("missing BPF function {name}"));
        let rest = &BPF_SOURCE[start..];
        let end = rest
            .find("\n}\n")
            .unwrap_or_else(|| panic!("unterminated BPF function {name}"));
        &rest[..end]
    }

    #[test]
    fn tx_send_correlation_is_scoped_to_one_send() {
        let begin = bpf_function("begin_send");
        let owner_reset = begin
            .find("bpf_map_delete_elem(&socket_owner, &sk_key);")
            .expect("begin_send must clear stale owner state");
        let ifindex_reset = begin
            .find("bpf_map_delete_elem(&socket_tx_ifindex, &sk_key);")
            .expect("begin_send must clear stale TX interface state");
        let owner_update = begin
            .find("bpf_map_update_elem(&socket_owner, &sk_key, &owner, BPF_ANY);")
            .expect("begin_send must register the current send owner");
        assert!(owner_reset < owner_update);
        assert!(ifindex_reset < owner_update);

        let finish = bpf_function("finish_send");
        assert!(finish.contains("if (bytes > 0 && owner && ifindex)"));
        assert!(finish.contains("bpf_map_delete_elem(&socket_owner, &sk_key);"));
        assert!(finish.contains("bpf_map_delete_elem(&socket_tx_ifindex, &sk_key);"));
        assert!(
            !finish.contains("if (!sk || bytes <= 0)"),
            "failed sends must still clear send-scoped correlation state"
        );
    }

    #[test]
    fn resolves_each_interface_identity_once_per_sample() {
        let mut identities = HashMap::new();
        let mut resolutions = 0;

        for ifindex in [7, 7, 9, 7, 9] {
            let resolved = resolve_identity(&mut identities, ifindex, |ifindex| {
                resolutions += 1;
                Some((
                    format!("if{ifindex}"),
                    network_id(&format!("network-{ifindex}")),
                ))
            });
            assert!(resolved.is_some());
        }

        assert_eq!(resolutions, 2);
    }

    #[test]
    fn caches_unresolved_identity_for_the_sample() {
        let mut identities = HashMap::new();
        let mut resolutions = 0;

        for _ in 0..3 {
            assert!(
                resolve_identity(&mut identities, 7, |_| {
                    resolutions += 1;
                    None
                })
                .is_none()
            );
        }

        assert_eq!(resolutions, 1);
    }

    #[test]
    fn merge_uses_stable_network_identity_not_display_name() {
        let mut rows = vec![
            ProcessNetworkIoCounter {
                process: ProcessInstanceId {
                    pid: 7,
                    birth_marker: 1,
                },
                network_id: network_id("network-a"),
                interface: "old-name".into(),
                rx_bytes: 11,
                tx_bytes: 0,
            },
            ProcessNetworkIoCounter {
                process: ProcessInstanceId {
                    pid: 7,
                    birth_marker: 1,
                },
                network_id: network_id("network-a"),
                interface: "new-name".into(),
                rx_bytes: 0,
                tx_bytes: 22,
            },
            ProcessNetworkIoCounter {
                process: ProcessInstanceId {
                    pid: 7,
                    birth_marker: 1,
                },
                network_id: network_id("network-b"),
                interface: "new-name".into(),
                rx_bytes: 33,
                tx_bytes: 0,
            },
        ];
        merge_rows(&mut rows);
        assert_eq!(rows.len(), 2);
        assert_eq!((rows[0].rx_bytes, rows[0].tx_bytes), (11, 22));
        assert_eq!(rows[0].network_id, network_id("network-a"));
        assert_eq!(rows[1].network_id, network_id("network-b"));
    }

    #[test]
    fn unresolved_native_ids_are_unavailable_not_fabricated_names() {
        assert_eq!(native::interface_name(u32::MAX), None);
    }
}
