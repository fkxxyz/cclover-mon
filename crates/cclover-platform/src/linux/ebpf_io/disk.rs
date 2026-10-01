use std::collections::{HashMap, HashSet};
use std::ffi::CStr;
use std::fs;
use std::mem::offset_of;
use std::path::Path;

use cclover_core::model::{DiskId, ProcessDiskIoCounter, ProcessInstanceId};

use super::abi;
use super::runtime::{LoadedObject, delete_map_key, read_map};
use super::{AttributionFailure, AttributionRows, FailureKind, is_stale_process};
use crate::linux::disk as disk_metric;
use crate::linux::process::birth_marker_from_start_boottime_ns;

const OBJECT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/disk_attribution.bpf.o"));
const MAP: &CStr = c"disk_bytes";

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Key {
    tgid: u32,
    dev: u32,
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
    assert!(size_of::<Key>() == abi::disk_key::SIZE);
    assert!(align_of::<Key>() == abi::disk_key::ALIGN);
    assert!(offset_of!(Key, tgid) == abi::disk_key::TGID_OFFSET);
    assert!(offset_of!(Key, dev) == abi::disk_key::DEV_OFFSET);
    assert!(offset_of!(Key, direction) == abi::disk_key::DIRECTION_OFFSET);
    assert!(offset_of!(Key, pad) == abi::disk_key::PAD_OFFSET);

    assert!(size_of::<CounterValue>() == abi::disk_counter_value::SIZE);
    assert!(align_of::<CounterValue>() == abi::disk_counter_value::ALIGN);
    assert!(
        offset_of!(CounterValue, process_start_time)
            == abi::disk_counter_value::PROCESS_START_TIME_OFFSET
    );
    assert!(offset_of!(CounterValue, bytes) == abi::disk_counter_value::BYTES_OFFSET);
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
    ) -> Result<AttributionRows<ProcessDiskIoCounter>, AttributionFailure> {
        let object = self.object()?;
        let map_fd = object.map_fd(MAP)?;
        let mut rows = Vec::new();
        let mut stale_keys = Vec::new();
        let mut unresolved_native_ids = 0;
        let mut resolved_devices = HashMap::new();
        for (key, value) in read_map::<Key, CounterValue>(map_fd)? {
            let process = ProcessInstanceId {
                pid: key.tgid,
                birth_marker: birth_marker_from_start_boottime_ns(value.process_start_time),
            };
            if is_stale_process(active_processes, &process) {
                stale_keys.push(key);
                continue;
            }
            let Some((disk_id, device)) = resolved_devices
                .entry(key.dev)
                .or_insert_with(|| resolve_device(key.dev))
                .as_ref()
            else {
                unresolved_native_ids += 1;
                continue;
            };
            rows.push(ProcessDiskIoCounter {
                process,
                disk_id: disk_id.clone(),
                device: device.clone(),
                read_bytes: if key.direction == 0 { value.bytes } else { 0 },
                write_bytes: if key.direction == 1 { value.bytes } else { 0 },
            });
        }
        for key in &stale_keys {
            delete_map_key(map_fd, key)?;
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
            .expect("disk attribution initialized")
            .as_ref()
            .map_err(Clone::clone)
    }
}

fn resolve_device(dev: u32) -> Option<(DiskId, String)> {
    resolve_device_from(dev, Path::new("/sys/dev/block"), Path::new("/sys/block"))
}

fn resolve_device_from(
    dev: u32,
    dev_block_root: &Path,
    block_root: &Path,
) -> Option<(DiskId, String)> {
    let (major, minor) = decode_kernel_dev(dev);
    let sys_path = dev_block_root.join(format!("{major}:{minor}"));
    let path = fs::canonicalize(&sys_path).ok()?;
    let device = block_name_from_sysfs_path(&path, sys_path.join("partition").is_file())?;
    let disk_id = disk_metric::id_for_name_from(block_root, &device)?;
    Some((disk_id, device))
}

fn decode_kernel_dev(dev: u32) -> (u32, u32) {
    (dev >> 20, dev & ((1 << 20) - 1))
}

fn block_name_from_sysfs_path(path: &Path, is_partition: bool) -> Option<String> {
    let identity = if is_partition { path.parent()? } else { path };
    identity
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

fn merge_rows(rows: &mut Vec<ProcessDiskIoCounter>) {
    rows.sort_by(|a, b| (a.process, &a.disk_id).cmp(&(b.process, &b.disk_id)));
    let mut merged: Vec<ProcessDiskIoCounter> = Vec::with_capacity(rows.len());
    for row in rows.drain(..) {
        if let Some(last) = merged.last_mut()
            && last.process == row.process
            && last.disk_id == row.disk_id
        {
            last.read_bytes = last.read_bytes.saturating_add(row.read_bytes);
            last.write_bytes = last.write_bytes.saturating_add(row.write_bytes);
        } else {
            merged.push(row);
        }
    }
    *rows = merged;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linux::test_support::Fixture;

    fn disk_id(key: &str) -> DiskId {
        DiskId::from_opaque_key(key)
    }

    #[test]
    fn merge_uses_stable_disk_identity_not_display_name() {
        let mut rows = vec![
            ProcessDiskIoCounter {
                process: ProcessInstanceId {
                    pid: 7,
                    birth_marker: 1,
                },
                disk_id: disk_id("disk-a"),
                device: "old-name".into(),
                read_bytes: 10,
                write_bytes: 0,
            },
            ProcessDiskIoCounter {
                process: ProcessInstanceId {
                    pid: 7,
                    birth_marker: 1,
                },
                disk_id: disk_id("disk-a"),
                device: "new-name".into(),
                read_bytes: 0,
                write_bytes: 20,
            },
            ProcessDiskIoCounter {
                process: ProcessInstanceId {
                    pid: 7,
                    birth_marker: 1,
                },
                disk_id: disk_id("disk-b"),
                device: "new-name".into(),
                read_bytes: 30,
                write_bytes: 0,
            },
        ];
        merge_rows(&mut rows);
        assert_eq!(rows.len(), 2);
        assert_eq!((rows[0].read_bytes, rows[0].write_bytes), (10, 20));
        assert_eq!(rows[0].disk_id, disk_id("disk-a"));
        assert_eq!(rows[1].disk_id, disk_id("disk-b"));
    }

    #[test]
    fn merge_does_not_cross_process_instances_that_reuse_a_pid() {
        let mut rows = vec![
            ProcessDiskIoCounter {
                process: ProcessInstanceId {
                    pid: 7,
                    birth_marker: 1,
                },
                disk_id: disk_id("disk-a"),
                device: "sda".into(),
                read_bytes: 10,
                write_bytes: 0,
            },
            ProcessDiskIoCounter {
                process: ProcessInstanceId {
                    pid: 7,
                    birth_marker: 2,
                },
                disk_id: disk_id("disk-a"),
                device: "sda".into(),
                read_bytes: 0,
                write_bytes: 20,
            },
        ];

        merge_rows(&mut rows);
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn resolves_partition_path_to_parent_block_device() {
        let path = Path::new("/sys/devices/pci0000:00/nvme/nvme1/nvme1n1/nvme1n1p4");
        assert_eq!(
            block_name_from_sysfs_path(path, true).as_deref(),
            Some("nvme1n1")
        );
        assert_eq!(
            block_name_from_sysfs_path(path, false).as_deref(),
            Some("nvme1n1p4")
        );
    }

    #[test]
    fn decodes_kernel_dev_t() {
        let dev = (43 << 20) | 7;
        assert_eq!(decode_kernel_dev(dev), (43, 7));
    }

    #[test]
    fn unresolved_native_ids_are_unavailable_not_fabricated_names() {
        assert_eq!(resolve_device(u32::MAX), None);
    }

    #[test]
    fn resolves_partition_native_id_to_parent_disk_identity_from_fixture() {
        let fixture = Fixture::new("ebpf-disk-identity");
        fixture.dir("dev/block");
        fixture.dir("devices/nvme1n1/nvme1n1p4");
        fixture.write("devices/nvme1n1/nvme1n1p4/partition", "4\n");
        fixture.symlink_to("devices/nvme1n1/nvme1n1p4", "dev/block/43:7");

        fixture.dir("devices/pci-nvme1");
        fixture.dir("block/nvme1n1");
        fixture.symlink_to("devices/pci-nvme1", "block/nvme1n1/device");
        fixture.write("block/nvme1n1/dev", "259:0\n");

        let resolved = resolve_device_from(
            (43 << 20) | 7,
            &fixture.path().join("dev/block"),
            &fixture.path().join("block"),
        )
        .expect("fixture device must resolve");

        assert_eq!(resolved.1, "nvme1n1");
        assert_eq!(
            resolved.0,
            disk_metric::id_for_name_from(&fixture.path().join("block"), "nvme1n1")
                .expect("fixture disk identity")
        );
    }
}
