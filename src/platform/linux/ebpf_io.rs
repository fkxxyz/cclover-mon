use std::ffi::{CStr, c_int, c_long, c_void};
use std::fmt;
use std::fs;
use std::mem::MaybeUninit;
use std::path::Path;
use std::ptr;

use crate::core::model::{ProcessDiskIoCounter, ProcessNetworkIoCounter};

const DISK_OBJECT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/disk_attribution.bpf.o"));
const NETWORK_OBJECT: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/network_attribution.bpf.o"));
const DISK_MAP: &CStr = c"disk_bytes";
const NETWORK_MAP: &CStr = c"network_bytes";

#[repr(C)]
struct BpfObject(c_void);
#[repr(C)]
struct BpfProgram(c_void);
#[repr(C)]
struct BpfMap(c_void);
#[repr(C)]
struct BpfLink(c_void);

unsafe extern "C" {
    fn bpf_object__open_mem(
        data: *const c_void,
        size: usize,
        opts: *const c_void,
    ) -> *mut BpfObject;
    fn bpf_object__load(obj: *mut BpfObject) -> c_int;
    fn bpf_object__close(obj: *mut BpfObject);
    fn bpf_object__next_program(obj: *const BpfObject, prev: *mut BpfProgram) -> *mut BpfProgram;
    fn bpf_program__attach(prog: *const BpfProgram) -> *mut BpfLink;
    fn bpf_link__destroy(link: *mut BpfLink) -> c_int;
    fn bpf_object__find_map_by_name(obj: *const BpfObject, name: *const i8) -> *mut BpfMap;
    fn bpf_map__fd(map: *const BpfMap) -> c_int;
    fn bpf_map_get_next_key(fd: c_int, key: *const c_void, next_key: *mut c_void) -> c_int;
    fn bpf_map_lookup_elem(fd: c_int, key: *const c_void, value: *mut c_void) -> c_int;
    fn libbpf_get_error(ptr: *const c_void) -> c_long;
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct DiskKey {
    tgid: u32,
    dev: u32,
    direction: u8,
    pad: [u8; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct NetworkKey {
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

struct LoadedObject {
    object: *mut BpfObject,
    links: Vec<*mut BpfLink>,
}

// libbpf handles are owned by Backend's collector thread and never escape the Linux backend.
unsafe impl Send for LoadedObject {}

impl LoadedObject {
    fn load(bytes: &[u8]) -> Result<Self, AttributionFailure> {
        if !Path::new("/sys/kernel/btf/vmlinux").is_file() {
            return Err(AttributionFailure::new(
                FailureKind::KernelBtf,
                "/sys/kernel/btf/vmlinux is unavailable",
            ));
        }

        let object =
            unsafe { bpf_object__open_mem(bytes.as_ptr().cast(), bytes.len(), ptr::null()) };
        check_ptr(
            object.cast(),
            FailureKind::Loader,
            "open embedded BPF object",
        )?;
        let mut loaded = Self {
            object,
            links: Vec::new(),
        };

        let rc = unsafe { bpf_object__load(loaded.object) };
        if rc != 0 {
            return Err(AttributionFailure::errno(
                FailureKind::KernelBtf,
                "load BPF object",
                rc,
            ));
        }
        loaded.attach_all()?;
        Ok(loaded)
    }

    fn attach_all(&mut self) -> Result<(), AttributionFailure> {
        let mut previous = ptr::null_mut();
        loop {
            let program = unsafe { bpf_object__next_program(self.object, previous) };
            if program.is_null() {
                break;
            }
            let link = unsafe { bpf_program__attach(program) };
            check_ptr(link.cast(), FailureKind::AttachPoint, "attach BPF program")?;
            self.links.push(link);
            previous = program;
        }
        if self.links.is_empty() {
            return Err(AttributionFailure::new(
                FailureKind::AttachPoint,
                "embedded object contains no attachable programs",
            ));
        }
        Ok(())
    }

    fn map_fd(&self, name: &CStr) -> Result<c_int, AttributionFailure> {
        let map = unsafe { bpf_object__find_map_by_name(self.object, name.as_ptr()) };
        if map.is_null() {
            return Err(AttributionFailure::new(
                FailureKind::MapAccess,
                format!("BPF map {:?} is missing", name),
            ));
        }
        let fd = unsafe { bpf_map__fd(map) };
        if fd < 0 {
            Err(AttributionFailure::errno(
                FailureKind::MapAccess,
                &format!("access BPF map {:?}", name),
                fd,
            ))
        } else {
            Ok(fd)
        }
    }
}

impl Drop for LoadedObject {
    fn drop(&mut self) {
        for link in self.links.drain(..).rev() {
            unsafe { bpf_link__destroy(link) };
        }
        unsafe { bpf_object__close(self.object) };
    }
}

fn check_ptr(
    ptr: *const c_void,
    kind: FailureKind,
    operation: &str,
) -> Result<(), AttributionFailure> {
    if ptr.is_null() {
        return Err(AttributionFailure::new(
            kind,
            format!("{operation}: null libbpf handle"),
        ));
    }
    let error = unsafe { libbpf_get_error(ptr) };
    if error == 0 {
        Ok(())
    } else {
        Err(AttributionFailure::errno(kind, operation, error as i32))
    }
}

pub(super) struct Collector {
    disabled: bool,
    disk: Option<Result<LoadedObject, AttributionFailure>>,
    network: Option<Result<LoadedObject, AttributionFailure>>,
}

impl Collector {
    pub(super) fn new() -> Self {
        Self {
            disabled: std::env::var_os("CCLOVER_MON_DISABLE_EBPF_IO").is_some(),
            disk: None,
            network: None,
        }
    }

    fn disk(&mut self) -> Result<&LoadedObject, AttributionFailure> {
        if self.disk.is_none() {
            self.disk = Some(if self.disabled {
                Err(AttributionFailure::new(
                    FailureKind::Disabled,
                    "CCLOVER_MON_DISABLE_EBPF_IO is set",
                ))
            } else {
                LoadedObject::load(DISK_OBJECT)
            });
        }
        self.disk
            .as_ref()
            .expect("disk attribution initialized")
            .as_ref()
            .map_err(Clone::clone)
    }

    fn network(&mut self) -> Result<&LoadedObject, AttributionFailure> {
        if self.network.is_none() {
            self.network = Some(if self.disabled {
                Err(AttributionFailure::new(
                    FailureKind::Disabled,
                    "CCLOVER_MON_DISABLE_EBPF_IO is set",
                ))
            } else {
                LoadedObject::load(NETWORK_OBJECT)
            });
        }
        self.network
            .as_ref()
            .expect("network attribution initialized")
            .as_ref()
            .map_err(Clone::clone)
    }

    pub(super) fn collect_disk(
        &mut self,
    ) -> Result<AttributionRows<ProcessDiskIoCounter>, AttributionFailure> {
        let object = self.disk()?;
        let mut rows = Vec::new();
        let mut unresolved_native_ids = 0;
        for (key, value) in read_map::<DiskKey>(object.map_fd(DISK_MAP)?)? {
            let Some(device) = resolve_device(key.dev) else {
                unresolved_native_ids += 1;
                continue;
            };
            rows.push(ProcessDiskIoCounter {
                pid: key.tgid,
                device,
                read_bytes: if key.direction == 0 { value.bytes } else { 0 },
                write_bytes: if key.direction == 1 { value.bytes } else { 0 },
            });
        }
        merge_disk_rows(&mut rows);
        Ok(AttributionRows {
            rows,
            unresolved_native_ids,
        })
    }

    pub(super) fn collect_network(
        &mut self,
    ) -> Result<AttributionRows<ProcessNetworkIoCounter>, AttributionFailure> {
        let object = self.network()?;
        let mut rows = Vec::new();
        let mut unresolved_native_ids = 0;
        for (key, value) in read_map::<NetworkKey>(object.map_fd(NETWORK_MAP)?)? {
            let Some(interface) = resolve_interface(key.ifindex) else {
                unresolved_native_ids += 1;
                continue;
            };
            rows.push(ProcessNetworkIoCounter {
                pid: key.tgid,
                interface,
                rx_bytes: if key.direction == 0 { value.bytes } else { 0 },
                tx_bytes: if key.direction == 1 { value.bytes } else { 0 },
            });
        }
        merge_network_rows(&mut rows);
        Ok(AttributionRows {
            rows,
            unresolved_native_ids,
        })
    }
}

fn read_map<K: Copy + Default>(fd: c_int) -> Result<Vec<(K, CounterValue)>, AttributionFailure> {
    let mut rows = Vec::new();
    let mut current: Option<K> = None;
    loop {
        let mut next = MaybeUninit::<K>::uninit();
        let rc = unsafe {
            bpf_map_get_next_key(
                fd,
                current
                    .as_ref()
                    .map_or(ptr::null(), |key| (key as *const K).cast()),
                next.as_mut_ptr().cast(),
            )
        };
        if rc != 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::ENOENT) {
                break;
            }
            return Err(AttributionFailure::new(
                FailureKind::MapAccess,
                format!("iterate BPF map: {error}"),
            ));
        }

        let next = unsafe { next.assume_init() };
        let mut value = CounterValue::default();
        let rc = unsafe {
            bpf_map_lookup_elem(
                fd,
                (&next as *const K).cast(),
                (&mut value as *mut CounterValue).cast(),
            )
        };
        if rc != 0 {
            let error = std::io::Error::last_os_error();
            current = Some(next);
            if error.raw_os_error() == Some(libc::ENOENT) {
                continue;
            }
            return Err(AttributionFailure::new(
                FailureKind::MapAccess,
                format!("read BPF map value: {error}"),
            ));
        }
        rows.push((next, value));
        current = Some(next);
    }
    Ok(rows)
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

fn resolve_interface(ifindex: u32) -> Option<String> {
    let mut name = [0_i8; libc::IF_NAMESIZE];
    let ptr = unsafe { libc::if_indextoname(ifindex, name.as_mut_ptr()) };
    if ptr.is_null() {
        return None;
    }
    Some(
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned(),
    )
}

fn merge_disk_rows(rows: &mut Vec<ProcessDiskIoCounter>) {
    rows.sort_by(|a, b| (a.pid, &a.device).cmp(&(b.pid, &b.device)));
    let mut merged: Vec<ProcessDiskIoCounter> = Vec::with_capacity(rows.len());
    for row in rows.drain(..) {
        if let Some(last) = merged.last_mut()
            && last.pid == row.pid
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

fn merge_network_rows(rows: &mut Vec<ProcessNetworkIoCounter>) {
    rows.sort_by(|a, b| (a.pid, &a.interface).cmp(&(b.pid, &b.interface)));
    let mut merged: Vec<ProcessNetworkIoCounter> = Vec::with_capacity(rows.len());
    for row in rows.drain(..) {
        if let Some(last) = merged.last_mut()
            && last.pid == row.pid
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
    fn merges_disk_directions_without_crossing_pid_or_device() {
        let mut rows = vec![
            ProcessDiskIoCounter {
                pid: 7,
                device: "sda".into(),
                read_bytes: 10,
                write_bytes: 0,
            },
            ProcessDiskIoCounter {
                pid: 7,
                device: "sda".into(),
                read_bytes: 0,
                write_bytes: 20,
            },
            ProcessDiskIoCounter {
                pid: 8,
                device: "sda".into(),
                read_bytes: 30,
                write_bytes: 0,
            },
        ];
        merge_disk_rows(&mut rows);
        assert_eq!(rows.len(), 2);
        assert_eq!((rows[0].read_bytes, rows[0].write_bytes), (10, 20));
        assert_eq!(rows[1].pid, 8);
    }

    #[test]
    fn merges_network_directions_without_crossing_pid_or_interface() {
        let mut rows = vec![
            ProcessNetworkIoCounter {
                pid: 7,
                interface: "lo".into(),
                rx_bytes: 11,
                tx_bytes: 0,
            },
            ProcessNetworkIoCounter {
                pid: 7,
                interface: "lo".into(),
                rx_bytes: 0,
                tx_bytes: 22,
            },
            ProcessNetworkIoCounter {
                pid: 7,
                interface: "eth0".into(),
                rx_bytes: 33,
                tx_bytes: 0,
            },
        ];
        merge_network_rows(&mut rows);
        assert_eq!(rows.len(), 2);
        assert!(
            rows.iter()
                .any(|row| row.interface == "lo" && row.rx_bytes == 11 && row.tx_bytes == 22)
        );
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
    fn classifies_loader_failures_independently_of_message_text() {
        let privilege = AttributionFailure::errno(FailureKind::KernelBtf, "load", -libc::EPERM);
        assert_eq!(privilege.kind, FailureKind::Privilege);

        let allocation = AttributionFailure::errno(FailureKind::KernelBtf, "load", -libc::ENOMEM);
        assert_eq!(allocation.kind, FailureKind::MapAccess);
    }

    #[test]
    fn unresolved_native_ids_are_unavailable_not_fabricated_names() {
        assert_eq!(resolve_device(u32::MAX), None);
        assert_eq!(resolve_interface(u32::MAX), None);
    }
}
