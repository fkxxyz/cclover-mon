use super::*;

impl Session {
    pub(in crate::windows) fn start(sink: Arc<dyn EventSink>) -> io::Result<Self> {
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

    pub(in crate::windows) fn health(&self) -> io::Result<Health> {
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

pub(super) struct OwnedHandle(HANDLE);

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
