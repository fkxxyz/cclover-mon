use super::*;

pub(in crate::windows) fn system_cpu_times() -> io::Result<CpuTimes> {
    let mut idle = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut kernel = idle;
    let mut user = idle;
    // SAFETY: all pointers refer to initialized writable FILETIME values.
    if unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: GetActiveProcessorCount has no pointer arguments and ALL_PROCESSOR_GROUPS is documented.
    let count = unsafe { GetActiveProcessorCount(ALL_PROCESSOR_GROUPS) } as usize;
    Ok(CpuTimes {
        idle: filetime(idle),
        kernel: filetime(kernel),
        user: filetime(user),
        logical_cpu_count: count,
    })
}

pub(in crate::windows) fn memory_status() -> io::Result<MemoryStatus> {
    let mut value = MEMORYSTATUSEX {
        dwLength: size_of::<MEMORYSTATUSEX>() as u32,
        dwMemoryLoad: 0,
        ullTotalPhys: 0,
        ullAvailPhys: 0,
        ullTotalPageFile: 0,
        ullAvailPageFile: 0,
        ullTotalVirtual: 0,
        ullAvailVirtual: 0,
        ullAvailExtendedVirtual: 0,
    };
    // SAFETY: value is a correctly sized writable MEMORYSTATUSEX.
    if unsafe { GlobalMemoryStatusEx(&mut value) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(MemoryStatus {
        total_phys: value.ullTotalPhys,
        avail_phys: value.ullAvailPhys,
    })
}

#[derive(Default)]
struct PageFileAccumulator {
    total_pages: u64,
    used_pages: u64,
    overflowed: bool,
}

unsafe extern "system" fn accumulate_page_file(
    context: *mut core::ffi::c_void,
    info: *mut ENUM_PAGE_FILE_INFORMATION,
    _filename: windows_sys::core::PCWSTR,
) -> BOOL {
    // SAFETY: K32EnumPageFilesW invokes this callback with the context pointer supplied by
    // page_file_status and a valid ENUM_PAGE_FILE_INFORMATION for the duration of the call.
    let accumulator = unsafe { &mut *context.cast::<PageFileAccumulator>() };
    // SAFETY: info is supplied by K32EnumPageFilesW and remains valid for this callback.
    let info = unsafe { &*info };

    match accumulator.total_pages.checked_add(info.TotalSize as u64) {
        Some(total) => accumulator.total_pages = total,
        None => accumulator.overflowed = true,
    }
    match accumulator.used_pages.checked_add(info.TotalInUse as u64) {
        Some(used) => accumulator.used_pages = used,
        None => accumulator.overflowed = true,
    }
    1
}

pub(in crate::windows) fn page_file_status() -> io::Result<PageFileStatus> {
    let mut system_info = SYSTEM_INFO::default();
    // SAFETY: system_info is a valid writable SYSTEM_INFO value.
    unsafe { GetSystemInfo(&mut system_info) };

    let mut accumulator = PageFileAccumulator::default();
    // SAFETY: the callback and context remain valid for the synchronous enumeration call.
    if unsafe {
        K32EnumPageFilesW(
            Some(accumulate_page_file),
            (&mut accumulator as *mut PageFileAccumulator).cast(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    if accumulator.overflowed {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "page-file page count overflow",
        ));
    }

    page_file_bytes(
        system_info.dwPageSize as u64,
        accumulator.total_pages,
        accumulator.used_pages,
    )
}

pub(in crate::windows) fn page_file_bytes(
    page_size: u64,
    total_pages: u64,
    used_pages: u64,
) -> io::Result<PageFileStatus> {
    let total_bytes = total_pages.checked_mul(page_size).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "page-file total byte overflow")
    })?;
    let used_bytes = used_pages.checked_mul(page_size).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "page-file used byte overflow")
    })?;
    Ok(PageFileStatus {
        total_bytes,
        used_bytes,
    })
}
