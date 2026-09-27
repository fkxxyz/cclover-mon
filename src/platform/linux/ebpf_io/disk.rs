use std::ffi::CStr;
use std::fs;
use std::path::Path;

use crate::core::model::{ProcessDiskIoCounter, ProcessInstanceId};

use super::runtime::{LoadedObject, read_map};
use super::{AttributionFailure, AttributionRows, FailureKind};
use crate::platform::linux::process::birth_marker_from_start_boottime_ns;

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
    ) -> Result<AttributionRows<ProcessDiskIoCounter>, AttributionFailure> {
        let object = self.object()?;
        let mut rows = Vec::new();
        let mut unresolved_native_ids = 0;
        for (key, value) in read_map::<Key, CounterValue>(object.map_fd(MAP)?)? {
            let Some(device) = resolve_device(key.dev) else {
                unresolved_native_ids += 1;
                continue;
            };
            rows.push(ProcessDiskIoCounter {
                process: ProcessInstanceId {
                    pid: key.tgid,
                    birth_marker: birth_marker_from_start_boottime_ns(value.process_start_time),
                },
                device,
                read_bytes: if key.direction == 0 { value.bytes } else { 0 },
                write_bytes: if key.direction == 1 { value.bytes } else { 0 },
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
            .expect("disk attribution initialized")
            .as_ref()
            .map_err(Clone::clone)
    }
}

fn resolve_device(dev: u32) -> Option<String> {
    let (major, minor) = decode_kernel_dev(dev);
    let sys_path = std::path::PathBuf::from(format!("/sys/dev/block/{major}:{minor}"));
    let path = fs::canonicalize(&sys_path).ok()?;
    block_name_from_sysfs_path(&path, sys_path.join("partition").is_file())
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
    rows.sort_by(|a, b| (a.process, &a.device).cmp(&(b.process, &b.device)));
    let mut merged: Vec<ProcessDiskIoCounter> = Vec::with_capacity(rows.len());
    for row in rows.drain(..) {
        if let Some(last) = merged.last_mut()
            && last.process == row.process
            && last.device == row.device
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

    #[test]
    fn merges_directions_without_crossing_pid_or_device() {
        let mut rows = vec![
            ProcessDiskIoCounter {
                process: ProcessInstanceId {
                    pid: 7,
                    birth_marker: 1,
                },
                device: "sda".into(),
                read_bytes: 10,
                write_bytes: 0,
            },
            ProcessDiskIoCounter {
                process: ProcessInstanceId {
                    pid: 7,
                    birth_marker: 1,
                },
                device: "sda".into(),
                read_bytes: 0,
                write_bytes: 20,
            },
            ProcessDiskIoCounter {
                process: ProcessInstanceId {
                    pid: 8,
                    birth_marker: 1,
                },
                device: "sda".into(),
                read_bytes: 30,
                write_bytes: 0,
            },
        ];
        merge_rows(&mut rows);
        assert_eq!(rows.len(), 2);
        assert_eq!((rows[0].read_bytes, rows[0].write_bytes), (10, 20));
        assert_eq!(rows[1].process.pid, 8);
    }

    #[test]
    fn merge_does_not_cross_process_instances_that_reuse_a_pid() {
        let mut rows = vec![
            ProcessDiskIoCounter {
                process: ProcessInstanceId {
                    pid: 7,
                    birth_marker: 1,
                },
                device: "sda".into(),
                read_bytes: 10,
                write_bytes: 0,
            },
            ProcessDiskIoCounter {
                process: ProcessInstanceId {
                    pid: 7,
                    birth_marker: 2,
                },
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
}
