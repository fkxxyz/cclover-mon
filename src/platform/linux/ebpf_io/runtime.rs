#![allow(unsafe_code)]

use std::ffi::{CStr, c_int, c_long, c_void};
use std::mem::MaybeUninit;
use std::path::Path;
use std::ptr;

use super::{AttributionFailure, FailureKind};

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

pub(super) struct LoadedObject {
    object: *mut BpfObject,
    links: Vec<*mut BpfLink>,
}

// SAFETY: LoadedObject uniquely owns the libbpf object and links. Moving ownership to another
// thread does not create concurrent access; callers can only access it through owned collectors.
unsafe impl Send for LoadedObject {}

impl LoadedObject {
    pub(super) fn load(bytes: &[u8]) -> Result<Self, AttributionFailure> {
        if !Path::new("/sys/kernel/btf/vmlinux").is_file() {
            return Err(AttributionFailure::new(
                FailureKind::KernelBtf,
                "/sys/kernel/btf/vmlinux is unavailable",
            ));
        }

        // SAFETY: bytes remains valid for this call and libbpf consumes it synchronously. A null
        // options pointer is explicitly accepted by bpf_object__open_mem.
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

        // SAFETY: loaded.object is a successfully opened object uniquely owned by loaded.
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
            // SAFETY: self.object remains live and previous is null or a program from this object.
            let program = unsafe { bpf_object__next_program(self.object, previous) };
            if program.is_null() {
                break;
            }
            // SAFETY: program belongs to the live loaded object.
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

    pub(super) fn map_fd(&self, name: &CStr) -> Result<c_int, AttributionFailure> {
        // SAFETY: self.object is live and name is a valid NUL-terminated C string.
        let map = unsafe { bpf_object__find_map_by_name(self.object, name.as_ptr()) };
        if map.is_null() {
            return Err(AttributionFailure::new(
                FailureKind::MapAccess,
                format!("BPF map {:?} is missing", name),
            ));
        }
        // SAFETY: map was returned from the live object and is non-null.
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
            // SAFETY: every link is uniquely owned by self and destroyed exactly once here.
            unsafe { bpf_link__destroy(link) };
        }
        // SAFETY: object is uniquely owned by self and closed exactly once after its links.
        unsafe { bpf_object__close(self.object) };
    }
}

pub(super) fn read_map<K, V>(fd: c_int) -> Result<Vec<(K, V)>, AttributionFailure>
where
    K: Copy + Default,
    V: Copy + Default,
{
    let mut rows = Vec::new();
    let mut current: Option<K> = None;
    loop {
        let mut next = MaybeUninit::<K>::uninit();
        // SAFETY: next points to writable K-sized storage; current, when present, points to a live
        // K. Callers select K to match the map key ABI.
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

        // SAFETY: successful bpf_map_get_next_key initializes the complete next key.
        let next = unsafe { next.assume_init() };
        let mut value = V::default();
        // SAFETY: next is a live key and value points to writable V-sized storage. Callers select
        // K and V to match the map ABI.
        let rc = unsafe {
            bpf_map_lookup_elem(
                fd,
                (&next as *const K).cast(),
                (&mut value as *mut V).cast(),
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
    // SAFETY: ptr came directly from a libbpf pointer-returning API and is non-null.
    let error = unsafe { libbpf_get_error(ptr) };
    if error == 0 {
        Ok(())
    } else {
        Err(AttributionFailure::errno(kind, operation, error as i32))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_loader_failures_independently_of_message_text() {
        let privilege = AttributionFailure::errno(FailureKind::KernelBtf, "load", -libc::EPERM);
        assert_eq!(privilege.kind, FailureKind::Privilege);

        let allocation = AttributionFailure::errno(FailureKind::KernelBtf, "load", -libc::ENOMEM);
        assert_eq!(allocation.kind, FailureKind::MapAccess);
    }
}
