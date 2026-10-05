#![allow(unsafe_code)]

use std::ffi::{CStr, c_int, c_long, c_void};
use std::mem::MaybeUninit;
use std::path::Path;
use std::ptr;
use std::sync::OnceLock;

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
    fn bpf_map_delete_elem(fd: c_int, key: *const c_void) -> c_int;
    fn libbpf_get_error(ptr: *const c_void) -> c_long;
}

type BpfMapLookupBatch = unsafe extern "C" fn(
    fd: c_int,
    in_batch: *mut c_void,
    out_batch: *mut c_void,
    keys: *mut c_void,
    values: *mut c_void,
    count: *mut u32,
    opts: *const c_void,
) -> c_int;

// Conservative syscall-amortization choice for map reads; not a libbpf or kernel ABI limit.
const MAP_BATCH_SIZE: usize = 64;
static BPF_MAP_LOOKUP_BATCH: OnceLock<Option<BpfMapLookupBatch>> = OnceLock::new();

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
    if let Some(rows) = try_read_map_batch(fd)? {
        return Ok(rows);
    }
    read_map_scalar(fd)
}

fn try_read_map_batch<K, V>(fd: c_int) -> Result<Option<Vec<(K, V)>>, AttributionFailure>
where
    K: Copy + Default,
    V: Copy + Default,
{
    let Some(lookup_batch) = lookup_batch_api() else {
        return Ok(None);
    };

    let mut rows = Vec::new();
    // Hash-family batch cursors require at least four bytes even when the map key is smaller.
    let mut cursor = vec![0_u8; size_of::<K>().max(size_of::<u32>())];
    let mut has_completed_batch = false;
    let mut keys = vec![K::default(); MAP_BATCH_SIZE];
    let mut values = vec![V::default(); MAP_BATCH_SIZE];

    loop {
        let mut count = MAP_BATCH_SIZE as u32;
        // SAFETY: cursor satisfies libbpf's key-sized/four-byte-minimum batch cursor contract;
        // keys and values each contain MAP_BATCH_SIZE
        // initialized elements whose layouts match the selected map ABI. A null opts pointer
        // selects default libbpf batch behavior.
        let rc = unsafe {
            lookup_batch(
                fd,
                if has_completed_batch {
                    cursor.as_mut_ptr().cast()
                } else {
                    ptr::null_mut()
                },
                cursor.as_mut_ptr().cast(),
                keys.as_mut_ptr().cast(),
                values.as_mut_ptr().cast(),
                &mut count,
                ptr::null(),
            )
        };

        if rc == 0 {
            append_batch_rows(&mut rows, &keys, &values, count)?;
            if count == 0 {
                return Err(AttributionFailure::new(
                    FailureKind::MapAccess,
                    "read BPF map batch: successful batch contained no rows",
                ));
            }
            has_completed_batch = true;
            continue;
        }

        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ENOENT) {
            // ENOENT terminates batch iteration but count still describes the final rows returned.
            append_batch_rows(&mut rows, &keys, &values, count)?;
            return Ok(Some(rows));
        }
        if should_fallback_batch_lookup(&error, has_completed_batch) {
            // No batch result has been consumed, so restarting through the scalar path cannot mix
            // two traversal strategies in one returned sample.
            return Ok(None);
        }
        return Err(AttributionFailure::new(
            FailureKind::MapAccess,
            format!("read BPF map batch: {error}"),
        ));
    }
}

fn append_batch_rows<K: Copy, V: Copy>(
    rows: &mut Vec<(K, V)>,
    keys: &[K],
    values: &[V],
    count: u32,
) -> Result<(), AttributionFailure> {
    let count = count as usize;
    if count > keys.len() || count > values.len() {
        return Err(AttributionFailure::new(
            FailureKind::MapAccess,
            format!("read BPF map batch: invalid returned row count {count}"),
        ));
    }
    rows.extend(
        keys[..count]
            .iter()
            .copied()
            .zip(values[..count].iter().copied()),
    );
    Ok(())
}

fn read_map_scalar<K, V>(fd: c_int) -> Result<Vec<(K, V)>, AttributionFailure>
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

pub(super) fn delete_map_keys<K>(fd: c_int, keys: &[K]) -> Result<(), AttributionFailure> {
    for key in keys {
        delete_map_key_scalar(fd, key)?;
    }
    Ok(())
}

fn delete_map_key_scalar<K>(fd: c_int, key: &K) -> Result<(), AttributionFailure> {
    // SAFETY: key points to a live K whose layout matches the map key ABI selected by the caller.
    let rc = unsafe { bpf_map_delete_elem(fd, (key as *const K).cast()) };
    if rc == 0 {
        return Ok(());
    }

    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ENOENT) {
        return Ok(());
    }
    Err(AttributionFailure::new(
        FailureKind::MapAccess,
        format!("delete BPF map key: {error}"),
    ))
}

fn lookup_batch_api() -> Option<BpfMapLookupBatch> {
    *BPF_MAP_LOOKUP_BATCH.get_or_init(|| {
        // SAFETY: libbpf is already linked for this module. dlsym either returns null or the
        // process-wide symbol address. The function type exactly matches libbpf's public ABI.
        let symbol = unsafe { libc::dlsym(libc::RTLD_DEFAULT, c"bpf_map_lookup_batch".as_ptr()) };
        if symbol.is_null() {
            None
        } else {
            // SAFETY: the symbol name identifies bpf_map_lookup_batch and the type above mirrors
            // its C declaration. Function pointers and data pointers have the same representation
            // on supported Linux targets.
            Some(unsafe { std::mem::transmute::<*mut c_void, BpfMapLookupBatch>(symbol) })
        }
    })
}

fn should_fallback_batch_lookup(error: &std::io::Error, has_completed_batch: bool) -> bool {
    !has_completed_batch && is_batch_unsupported(error)
}

fn is_batch_unsupported(error: &std::io::Error) -> bool {
    matches!(
        error.raw_os_error(),
        Some(code) if matches!(code, libc::EINVAL | libc::EOPNOTSUPP | libc::ENOSYS)
    )
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

    #[test]
    fn batch_unsupported_classification_is_narrow() {
        for errno in [libc::EINVAL, libc::EOPNOTSUPP, libc::ENOSYS] {
            assert!(is_batch_unsupported(&std::io::Error::from_raw_os_error(
                errno
            )));
        }
        for errno in [libc::EBADF, libc::EFAULT, libc::EPERM, libc::ENOMEM] {
            assert!(!is_batch_unsupported(&std::io::Error::from_raw_os_error(
                errno
            )));
        }
    }

    #[test]
    fn batch_lookup_only_falls_back_before_any_completed_batch() {
        let unsupported = std::io::Error::from_raw_os_error(libc::EOPNOTSUPP);
        assert!(should_fallback_batch_lookup(&unsupported, false));
        assert!(!should_fallback_batch_lookup(&unsupported, true));

        let real_failure = std::io::Error::from_raw_os_error(libc::EPERM);
        assert!(!should_fallback_batch_lookup(&real_failure, false));
    }
}
