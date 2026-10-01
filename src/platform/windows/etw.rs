#![allow(unsafe_code)]

use std::io;
use std::mem::{size_of, zeroed};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr::null;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::sync_channel;
use std::thread::{self, JoinHandle};

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, ERROR_CANCELLED, ERROR_SUCCESS,
    ERROR_WMI_INSTANCE_NOT_FOUND, GetLastError, HANDLE,
};
use windows_sys::Win32::System::Diagnostics::Etw::{
    CONTROLTRACE_HANDLE, CloseTrace, ControlTraceW, EVENT_RECORD, EVENT_TRACE_CONTROL_STOP,
    EVENT_TRACE_FLAG_DISK_FILE_IO, EVENT_TRACE_FLAG_FILE_IO, EVENT_TRACE_FLAG_FILE_IO_INIT,
    EVENT_TRACE_FLAG_THREAD, EVENT_TRACE_LOGFILEW, EVENT_TRACE_PROPERTIES,
    EVENT_TRACE_REAL_TIME_MODE, EVENT_TRACE_SYSTEM_LOGGER_MODE, FileIoGuid, OpenTraceW,
    PROCESS_TRACE_MODE_EVENT_RECORD, PROCESS_TRACE_MODE_REAL_TIME, PROPERTY_DATA_DESCRIPTOR,
    ProcessTrace, QueryTraceW, StartTraceW, TdhGetProperty, TdhGetPropertySize, ThreadGuid,
    WNODE_FLAG_TRACED_GUID,
};
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::core::{GUID, PCWSTR};

const SESSION_NAME: &str = "cclover-mon disk attribution";
const OWNERSHIP_MUTEX_NAME: &str = "Global\\cclover-mon disk attribution owner";
const SESSION_GUID: GUID = GUID::from_u128(0x8c8d7c8f_76cb_41bd_9f61_c8701b899e02);
const MAX_PROPERTY_BYTES: u32 = 64 * 1024;

const FILE_NAME: u8 = 0;
const FILE_CREATE_NAME: u8 = 32;
const FILE_DELETE_NAME: u8 = 35;
const FILE_RUNDOWN: u8 = 36;
const FILE_CREATE: u8 = 64;
const FILE_CLEANUP: u8 = 65;
const FILE_CLOSE: u8 = 66;
const FILE_READ: u8 = 67;
const FILE_WRITE: u8 = 68;
const FILE_OPERATION_END: u8 = 76;

const THREAD_START: u8 = 1;
const THREAD_END: u8 = 2;
const THREAD_DC_START: u8 = 3;
const THREAD_DC_END: u8 = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Direction {
    Read,
    Write,
}

#[derive(Debug)]
pub(super) enum Event {
    ThreadStart {
        pid: u32,
        tid: u32,
    },
    ThreadEnd {
        tid: u32,
    },
    FileName {
        key: u64,
        path: String,
    },
    FileCreate {
        file_object: u64,
        path: String,
    },
    FileObjectEnd {
        file_object: u64,
    },
    FileKeyEnd {
        key: u64,
    },
    IoStart {
        irp: u64,
        tid: u32,
        file_object: u64,
        file_key: u64,
        direction: Direction,
        requested_bytes: u32,
        irp_flags: u32,
    },
    IoEnd {
        irp: u64,
        extra_info: u64,
        status: u32,
    },
}

pub(super) trait EventSink: Send + Sync + 'static {
    fn on_event(&self, event: Event);
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Health {
    pub(super) events_lost: u64,
    pub(super) decode_errors: u64,
    pub(super) consumer_failed: bool,
}

struct CallbackContext {
    sink: Arc<dyn EventSink>,
    decode_errors: AtomicU64,
    consumer_failed: AtomicBool,
}

pub(super) struct Session {
    control: CONTROLTRACE_HANDLE,
    _ownership_lease: OwnedHandle,
    callback: Arc<CallbackContext>,
    consumer: Option<JoinHandle<()>>,
}

impl Session {
    pub(super) fn start(sink: Arc<dyn EventSink>) -> io::Result<Self> {
        let ownership_lease = acquire_ownership()?;
        let control = match start_controller() {
            Ok(control) => control,
            Err(error) => return Err(error),
        };
        let callback = Arc::new(CallbackContext {
            sink,
            decode_errors: AtomicU64::new(0),
            consumer_failed: AtomicBool::new(false),
        });
        let thread_callback = callback.clone();
        let (ready_tx, ready_rx) = sync_channel::<Result<(), i32>>(1);
        let consumer = thread::Builder::new()
            .name("cclover-etw-disk".to_owned())
            .spawn(move || consume(thread_callback, ready_tx))
            .map_err(|error| {
                let _ = stop_controller(control);
                error
            })?;
        match ready_rx.recv() {
            Ok(Ok(())) => {}
            Ok(Err(code)) => {
                let _ = stop_controller(control);
                let _ = consumer.join();
                return Err(io::Error::from_raw_os_error(code));
            }
            Err(_) => {
                let _ = stop_controller(control);
                let _ = consumer.join();
                return Err(io::Error::other(
                    "ETW consumer exited before becoming ready",
                ));
            }
        }
        Ok(Self {
            control,
            _ownership_lease: ownership_lease,
            callback,
            consumer: Some(consumer),
        })
    }

    pub(super) fn health(&self) -> io::Result<Health> {
        let properties = query_properties(self.control)?;
        Ok(Health {
            events_lost: u64::from(properties.EventsLost)
                .saturating_add(u64::from(properties.LogBuffersLost))
                .saturating_add(u64::from(properties.RealTimeBuffersLost)),
            decode_errors: self.callback.decode_errors.load(Ordering::Relaxed),
            consumer_failed: self.callback.consumer_failed.load(Ordering::Acquire),
        })
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = stop_controller(self.control);
        if let Some(consumer) = self.consumer.take() {
            let _ = consumer.join();
        }
    }
}

struct OwnedHandle(HANDLE);

// Win32 kernel handles may be closed from a thread other than the thread that created them. This
// wrapper does not represent mutex ownership; it only keeps the named kernel object alive.
unsafe impl Send for OwnedHandle {}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: this wrapper owns exactly one live kernel handle.
            unsafe { CloseHandle(self.0) };
        }
    }
}

fn acquire_ownership() -> io::Result<OwnedHandle> {
    let name = wide_z(OWNERSHIP_MUTEX_NAME);
    // SAFETY: no security attributes are supplied, the mutex is not acquired, and name is
    // nul-terminated. Holding this handle keeps the named object alive for process lifetime.
    let mutex = unsafe { CreateMutexW(null(), 0, name.as_ptr()) };
    if mutex.is_null() {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: GetLastError reads thread-local Win32 error state immediately after CreateMutexW.
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        // Another live cclover-mon process holds the lease. Do not stop its fixed ETW session.
        // SAFETY: mutex is a live handle created above and is not otherwise owned.
        unsafe { CloseHandle(mutex) };
        return Err(io::Error::from_raw_os_error(ERROR_ALREADY_EXISTS as i32));
    }
    Ok(OwnedHandle(mutex))
}

fn start_controller() -> io::Result<CONTROLTRACE_HANDLE> {
    let name = wide_z(SESSION_NAME);
    let mut properties = PropertiesBuffer::new(&name);
    properties.configure_for_start();
    let mut control = CONTROLTRACE_HANDLE::default();
    // SAFETY: name is nul-terminated and properties owns a correctly aligned writable buffer.
    let mut status = unsafe { StartTraceW(&mut control, name.as_ptr(), properties.as_mut_ptr()) };
    if status == ERROR_ALREADY_EXISTS {
        if orphan_is_ours(&name)? {
            let _ = stop_by_name(&name);
            properties.reset(&name);
            properties.configure_for_start();
            // SAFETY: same arguments as the initial StartTraceW call after stopping our stale session.
            status = unsafe { StartTraceW(&mut control, name.as_ptr(), properties.as_mut_ptr()) };
        }
    }
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    Ok(control)
}

fn stop_controller(control: CONTROLTRACE_HANDLE) -> io::Result<()> {
    let name = wide_z(SESSION_NAME);
    let mut properties = PropertiesBuffer::new(&name);
    // SAFETY: control is the session handle returned by StartTraceW; buffer is writable.
    let status = unsafe {
        ControlTraceW(
            control,
            name.as_ptr(),
            properties.as_mut_ptr(),
            EVENT_TRACE_CONTROL_STOP,
        )
    };
    if matches!(status, ERROR_SUCCESS | ERROR_WMI_INSTANCE_NOT_FOUND) {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(status as i32))
    }
}

fn stop_by_name(name: &[u16]) -> io::Result<()> {
    let mut properties = PropertiesBuffer::new(name);
    // SAFETY: zero handle selects the named session and buffer is writable.
    let status = unsafe {
        ControlTraceW(
            CONTROLTRACE_HANDLE::default(),
            name.as_ptr(),
            properties.as_mut_ptr(),
            EVENT_TRACE_CONTROL_STOP,
        )
    };
    if matches!(status, ERROR_SUCCESS | ERROR_WMI_INSTANCE_NOT_FOUND) {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(status as i32))
    }
}

fn orphan_is_ours(name: &[u16]) -> io::Result<bool> {
    let mut properties = PropertiesBuffer::new(name);
    // SAFETY: zero handle selects the named session and buffer is writable.
    let status = unsafe {
        QueryTraceW(
            CONTROLTRACE_HANDLE::default(),
            name.as_ptr(),
            properties.as_mut_ptr(),
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    // SAFETY: QueryTraceW initialized EVENT_TRACE_PROPERTIES at the buffer start.
    Ok(unsafe { guid_eq(&(*properties.as_mut_ptr()).Wnode.Guid, &SESSION_GUID) })
}

fn query_properties(control: CONTROLTRACE_HANDLE) -> io::Result<EVENT_TRACE_PROPERTIES> {
    let name = wide_z(SESSION_NAME);
    let mut properties = PropertiesBuffer::new(&name);
    // SAFETY: control names a live session and buffer is writable.
    let status = unsafe { QueryTraceW(control, name.as_ptr(), properties.as_mut_ptr()) };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    // SAFETY: QueryTraceW initialized the fixed header at the buffer start.
    Ok(unsafe { *properties.as_mut_ptr() })
}

fn consume(callback: Arc<CallbackContext>, ready: std::sync::mpsc::SyncSender<Result<(), i32>>) {
    let name = wide_z(SESSION_NAME);
    let mut logfile: EVENT_TRACE_LOGFILEW = unsafe { zeroed() };
    logfile.LoggerName = name.as_ptr() as *mut u16;
    logfile.Anonymous1.ProcessTraceMode =
        PROCESS_TRACE_MODE_REAL_TIME | PROCESS_TRACE_MODE_EVENT_RECORD;
    logfile.Anonymous2.EventRecordCallback = Some(event_record_callback);
    logfile.Context = Arc::as_ptr(&callback) as *mut core::ffi::c_void;

    // SAFETY: logfile and its pointed-to name/context stay alive for the whole ProcessTrace call.
    let trace = unsafe { OpenTraceW(&mut logfile) };
    if trace.Value == u64::MAX {
        // SAFETY: GetLastError is read immediately after the failed OpenTraceW call.
        let code = unsafe { GetLastError() } as i32;
        let _ = ready.send(Err(code));
        callback.consumer_failed.store(true, Ordering::Release);
        return;
    }
    let _ = ready.send(Ok(()));
    // SAFETY: trace is a valid consumer handle and remains open until ProcessTrace returns.
    let status = unsafe { ProcessTrace(&trace, 1, null(), null()) };
    if status != ERROR_SUCCESS && status != ERROR_CANCELLED {
        callback.consumer_failed.store(true, Ordering::Release);
    }
    // SAFETY: trace was returned by OpenTraceW and is closed exactly once.
    unsafe { CloseTrace(trace) };
}

unsafe extern "system" fn event_record_callback(record: *mut EVENT_RECORD) {
    if record.is_null() {
        return;
    }
    // SAFETY: ETW owns record for callback duration; UserContext is the Arc pointee supplied to OpenTraceW.
    let context = unsafe { &*((*record).UserContext.cast::<CallbackContext>()) };
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: record remains valid for this callback invocation.
        match unsafe { decode_event(&*record) } {
            Ok(Some(event)) => context.sink.on_event(event),
            Ok(None) => {}
            Err(()) => {
                context.decode_errors.fetch_add(1, Ordering::Relaxed);
            }
        }
    }));
    if outcome.is_err() {
        context.consumer_failed.store(true, Ordering::Release);
    }
}

unsafe fn decode_event(record: &EVENT_RECORD) -> Result<Option<Event>, ()> {
    let opcode = record.EventHeader.EventDescriptor.Opcode;
    if guid_eq(&record.EventHeader.ProviderId, &ThreadGuid) {
        return match opcode {
            THREAD_START | THREAD_DC_START => Ok(Some(Event::ThreadStart {
                pid: property_u32(record, windows_sys::core::w!("ProcessId"))?,
                tid: property_u32(record, windows_sys::core::w!("TThreadId"))?,
            })),
            THREAD_END | THREAD_DC_END => Ok(Some(Event::ThreadEnd {
                tid: property_u32(record, windows_sys::core::w!("TThreadId"))?,
            })),
            _ => Ok(None),
        };
    }
    if !guid_eq(&record.EventHeader.ProviderId, &FileIoGuid) {
        return Ok(None);
    }

    match opcode {
        FILE_NAME | FILE_CREATE_NAME | FILE_RUNDOWN => Ok(Some(Event::FileName {
            key: property_pointer(record, windows_sys::core::w!("FileObject"))?,
            path: property_string(record, windows_sys::core::w!("FileName"))?,
        })),
        // FileIo_Name uses the historical FileObject property name for the file-key value.
        FILE_DELETE_NAME => Ok(Some(Event::FileKeyEnd {
            key: property_pointer(record, windows_sys::core::w!("FileObject"))?,
        })),
        FILE_CREATE => Ok(Some(Event::FileCreate {
            file_object: property_pointer(record, windows_sys::core::w!("FileObject"))?,
            path: property_string(record, windows_sys::core::w!("OpenPath"))?,
        })),
        FILE_CLEANUP | FILE_CLOSE => Ok(Some(Event::FileObjectEnd {
            file_object: property_pointer(record, windows_sys::core::w!("FileObject"))?,
        })),
        FILE_READ | FILE_WRITE => Ok(Some(Event::IoStart {
            irp: property_pointer(record, windows_sys::core::w!("IrpPtr"))?,
            tid: property_pointer(record, windows_sys::core::w!("TTID"))? as u32,
            file_object: property_pointer(record, windows_sys::core::w!("FileObject"))?,
            file_key: property_pointer(record, windows_sys::core::w!("FileKey"))?,
            direction: if opcode == FILE_READ {
                Direction::Read
            } else {
                Direction::Write
            },
            requested_bytes: property_u32(record, windows_sys::core::w!("IoSize"))?,
            irp_flags: property_u32(record, windows_sys::core::w!("IoFlags"))?,
        })),
        FILE_OPERATION_END => Ok(Some(Event::IoEnd {
            irp: property_pointer(record, windows_sys::core::w!("IrpPtr"))?,
            extra_info: property_pointer(record, windows_sys::core::w!("ExtraInfo"))?,
            status: property_u32(record, windows_sys::core::w!("NtStatus"))?,
        })),
        _ => Ok(None),
    }
}

fn property_descriptor(name: PCWSTR) -> PROPERTY_DATA_DESCRIPTOR {
    PROPERTY_DATA_DESCRIPTOR {
        PropertyName: name as usize as u64,
        ArrayIndex: u32::MAX,
        Reserved: 0,
    }
}

fn property_u32(record: &EVENT_RECORD, name: PCWSTR) -> Result<u32, ()> {
    let bytes = property_fixed::<4>(record, name)?;
    Ok(u32::from_le_bytes(bytes))
}

fn property_pointer(record: &EVENT_RECORD, name: PCWSTR) -> Result<u64, ()> {
    let descriptor = property_descriptor(name);
    let mut size = 0_u32;
    // SAFETY: descriptor points to a static nul-terminated property name and size is writable.
    let status = unsafe { TdhGetPropertySize(record, 0, null(), 1, &descriptor, &mut size) };
    if status != ERROR_SUCCESS || !matches!(size, 4 | 8) {
        return Err(());
    }
    let mut bytes = [0_u8; 8];
    // SAFETY: bytes is writable for size bytes and descriptor remains valid.
    let status =
        unsafe { TdhGetProperty(record, 0, null(), 1, &descriptor, size, bytes.as_mut_ptr()) };
    if status != ERROR_SUCCESS {
        return Err(());
    }
    Ok(u64::from_le_bytes(bytes))
}

fn property_fixed<const N: usize>(record: &EVENT_RECORD, name: PCWSTR) -> Result<[u8; N], ()> {
    let descriptor = property_descriptor(name);
    let mut bytes = [0_u8; N];
    // SAFETY: bytes is exactly the expected property size and descriptor uses a static name.
    let status = unsafe {
        TdhGetProperty(
            record,
            0,
            null(),
            1,
            &descriptor,
            N as u32,
            bytes.as_mut_ptr(),
        )
    };
    if status == ERROR_SUCCESS {
        Ok(bytes)
    } else {
        Err(())
    }
}

fn property_string(record: &EVENT_RECORD, name: PCWSTR) -> Result<String, ()> {
    let descriptor = property_descriptor(name);
    let mut size = 0_u32;
    // SAFETY: descriptor uses a static property name and size is writable.
    let status = unsafe { TdhGetPropertySize(record, 0, null(), 1, &descriptor, &mut size) };
    if status != ERROR_SUCCESS || size == 0 || size > MAX_PROPERTY_BYTES || size % 2 != 0 {
        return Err(());
    }
    let mut bytes = vec![0_u8; size as usize];
    // SAFETY: bytes is writable for size bytes and descriptor remains valid.
    let status =
        unsafe { TdhGetProperty(record, 0, null(), 1, &descriptor, size, bytes.as_mut_ptr()) };
    if status != ERROR_SUCCESS {
        return Err(());
    }
    let units = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .take_while(|unit| *unit != 0)
        .collect::<Vec<_>>();
    String::from_utf16(&units).map_err(|_| ())
}

struct PropertiesBuffer {
    words: Vec<u64>,
    bytes: usize,
}

impl PropertiesBuffer {
    fn new(name: &[u16]) -> Self {
        let bytes = size_of::<EVENT_TRACE_PROPERTIES>() + name.len() * size_of::<u16>();
        let words = bytes.div_ceil(size_of::<u64>());
        let mut this = Self {
            words: vec![0_u64; words],
            bytes,
        };
        this.reset(name);
        this
    }

    fn reset(&mut self, name: &[u16]) {
        self.words.fill(0);
        let properties = self.as_mut_ptr();
        // SAFETY: buffer is aligned and large enough for EVENT_TRACE_PROPERTIES plus name.
        unsafe {
            (*properties).Wnode.BufferSize = self.bytes as u32;
            (*properties).Wnode.Flags = WNODE_FLAG_TRACED_GUID;
            (*properties).Wnode.Guid = SESSION_GUID;
            (*properties).Wnode.ClientContext = 1;
            (*properties).LoggerNameOffset = size_of::<EVENT_TRACE_PROPERTIES>() as u32;
            let dst = (properties as *mut u8)
                .add((*properties).LoggerNameOffset as usize)
                .cast::<u16>();
            std::ptr::copy_nonoverlapping(name.as_ptr(), dst, name.len());
        }
    }

    fn configure_for_start(&mut self) {
        let properties = self.as_mut_ptr();
        // SAFETY: properties points to initialized EVENT_TRACE_PROPERTIES in our owned buffer.
        unsafe {
            (*properties).LogFileMode = EVENT_TRACE_REAL_TIME_MODE | EVENT_TRACE_SYSTEM_LOGGER_MODE;
            (*properties).EnableFlags = EVENT_TRACE_FLAG_THREAD
                | EVENT_TRACE_FLAG_DISK_FILE_IO
                | EVENT_TRACE_FLAG_FILE_IO
                | EVENT_TRACE_FLAG_FILE_IO_INIT;
            (*properties).BufferSize = 64;
            (*properties).MinimumBuffers = 4;
            (*properties).MaximumBuffers = 32;
            (*properties).FlushTimer = 1;
        }
    }

    fn as_mut_ptr(&mut self) -> *mut EVENT_TRACE_PROPERTIES {
        self.words.as_mut_ptr().cast()
    }
}

fn wide_z(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn guid_eq(left: &GUID, right: &GUID) -> bool {
    left.data1 == right.data1
        && left.data2 == right.data2
        && left.data3 == right.data3
        && left.data4 == right.data4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn properties_buffer_keeps_session_name_after_header() {
        let name = wide_z("test-session");
        let mut buffer = PropertiesBuffer::new(&name);
        let properties = buffer.as_mut_ptr();
        // SAFETY: PropertiesBuffer owns the complete header + UTF-16 name allocation.
        let stored = unsafe {
            let ptr = (properties as *const u8)
                .add((*properties).LoggerNameOffset as usize)
                .cast::<u16>();
            std::slice::from_raw_parts(ptr, name.len())
        };
        assert_eq!(stored, name.as_slice());
    }
}
