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

mod decode;
mod properties;
mod session;

use decode::decode_event;
use properties::{PropertiesBuffer, guid_eq, wide_z};
use session::OwnedHandle;

#[cfg(test)]
mod tests;
