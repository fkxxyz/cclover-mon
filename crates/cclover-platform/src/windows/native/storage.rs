use super::*;

pub(in crate::windows) fn physical_disks() -> io::Result<NativeDisks> {
    let mut disks = Vec::new();
    let mut fallback_identities = 0;
    let mut first_error = None;
    for index in 0..MAX_PHYSICAL_DRIVES {
        let path = format!(r"\\.\PhysicalDrive{index}");
        let wide = wide_z(&path);
        // SAFETY: wide is nul-terminated; no security attributes/template; share flags permit coexistence.
        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                null(),
                OPEN_EXISTING,
                0,
                null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            let error = io::Error::last_os_error();
            if !matches!(error.raw_os_error(), Some(2 | 3)) && first_error.is_none() {
                first_error = Some(error);
            }
            continue;
        }
        let performance = disk_performance(handle);
        let descriptor = storage_descriptor(handle);
        // SAFETY: handle was opened successfully and is closed exactly once here.
        unsafe { CloseHandle(handle) };
        let performance = match performance {
            Ok(performance) => performance,
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
                continue;
            }
        };
        let (identity, fallback) = descriptor.unwrap_or_else(|| {
            (
                format!("physical-drive:{}", performance.StorageDeviceNumber),
                true,
            )
        });
        fallback_identities += usize::from(fallback);
        disks.push(NativeDisk {
            disk_number: performance.StorageDeviceNumber,
            identity,
            read_bytes: performance.BytesRead.max(0) as u64,
            write_bytes: performance.BytesWritten.max(0) as u64,
        });
    }
    if disks.is_empty()
        && let Some(error) = first_error
    {
        return Err(error);
    }
    Ok(NativeDisks {
        disks,
        fallback_identities,
    })
}

pub(in crate::windows) fn drive_bindings() -> io::Result<DriveBindings> {
    // SAFETY: GetLogicalDrives has no pointer arguments.
    let mask = unsafe { GetLogicalDrives() };
    if mask == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut bindings = Vec::new();
    let mut failures = Vec::new();
    for index in 0..26_u32 {
        if mask & (1_u32 << index) == 0 {
            continue;
        }
        let letter = (b'A' + index as u8) as char;
        let label = format!("{letter}:");
        let root = wide_z(&format!("{label}\\"));
        // SAFETY: root is a nul-terminated drive-root path.
        let drive_type = unsafe { GetDriveTypeW(root.as_ptr()) };
        if !matches!(drive_type, DRIVE_FIXED | DRIVE_REMOVABLE) {
            continue;
        }
        let path = format!(r"\\.\{label}");
        let wide = wide_z(&path);
        // SAFETY: wide is nul-terminated; zero desired access is sufficient for this metadata query.
        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                null(),
                OPEN_EXISTING,
                0,
                null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            failures.push(DriveBindingFailure {
                label,
                stage: DriveBindingStage::OpenVolume,
                error: io::Error::last_os_error(),
            });
            continue;
        }
        let disk_numbers = volume_disk_numbers(handle);
        // SAFETY: handle was opened successfully and is closed exactly once here.
        unsafe { CloseHandle(handle) };
        match disk_numbers {
            Ok(disk_numbers) if !disk_numbers.is_empty() => {
                bindings.push(DriveBinding {
                    label,
                    disk_numbers,
                });
            }
            Ok(_) => {}
            Err(error) => failures.push(DriveBindingFailure {
                label,
                stage: DriveBindingStage::QueryExtents,
                error,
            }),
        }
    }
    Ok(DriveBindings { bindings, failures })
}

pub(in crate::windows) fn volume_disk_bindings() -> io::Result<VolumeDiskBindings> {
    const VOLUME_NAME_CAPACITY: usize = 1024;
    const DEVICE_NAME_CAPACITY: usize = 4096;
    const ERROR_NO_MORE_FILES: i32 = 18;

    let mut volume_name = vec![0_u16; VOLUME_NAME_CAPACITY];
    // SAFETY: volume_name is writable for the advertised number of UTF-16 code units.
    let find = unsafe { FindFirstVolumeW(volume_name.as_mut_ptr(), volume_name.len() as u32) };
    if find == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }

    let mut bindings = Vec::new();
    let mut failures = 0_usize;
    let enumeration = loop {
        let Some(volume_guid_path) = utf16_buffer(&volume_name) else {
            break Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "volume enumeration returned non-terminated name",
            ));
        };
        match volume_disk_binding(&volume_guid_path, DEVICE_NAME_CAPACITY) {
            Ok(binding) => bindings.push(binding),
            Err(_) => failures = failures.saturating_add(1),
        }

        volume_name.fill(0);
        // SAFETY: find is a live volume-enumeration handle and volume_name is writable.
        if unsafe { FindNextVolumeW(find, volume_name.as_mut_ptr(), volume_name.len() as u32) } == 0
        {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(ERROR_NO_MORE_FILES) {
                break Ok(());
            }
            break Err(error);
        }
    };
    // SAFETY: find was returned by FindFirstVolumeW and is closed exactly once.
    unsafe { FindVolumeClose(find) };
    enumeration?;
    bindings.sort_by(|a, b| a.nt_path.cmp(&b.nt_path));
    bindings.dedup_by(|a, b| a.nt_path == b.nt_path && a.disk_numbers == b.disk_numbers);
    Ok(VolumeDiskBindings { bindings, failures })
}

fn volume_disk_binding(
    volume_guid_path: &str,
    device_name_capacity: usize,
) -> io::Result<VolumeDiskBinding> {
    let open_path = volume_guid_path.trim_end_matches('\\');
    let open_wide = wide_z(open_path);
    // SAFETY: open_wide is nul-terminated; zero desired access is sufficient for extent metadata.
    let handle = unsafe {
        CreateFileW(
            open_wide.as_ptr(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            null(),
            OPEN_EXISTING,
            0,
            null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let disk_numbers = volume_disk_numbers(handle);
    // SAFETY: handle was opened successfully and is closed exactly once here.
    unsafe { CloseHandle(handle) };
    let disk_numbers = disk_numbers?;

    let dos_name = open_path
        .strip_prefix(r"\\?\")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid volume GUID path"))?;
    let dos_wide = wide_z(dos_name);
    let mut target = vec![0_u16; device_name_capacity];
    // SAFETY: dos_wide is nul-terminated and target is writable for the advertised size.
    let len =
        unsafe { QueryDosDeviceW(dos_wide.as_ptr(), target.as_mut_ptr(), target.len() as u32) };
    if len == 0 {
        return Err(io::Error::last_os_error());
    }
    let nt_path = utf16_buffer(&target[..len as usize]).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "QueryDosDevice returned non-terminated target",
        )
    })?;
    Ok(VolumeDiskBinding {
        nt_path,
        disk_numbers,
    })
}

fn utf16_buffer(buffer: &[u16]) -> Option<String> {
    let end = buffer.iter().position(|unit| *unit == 0)?;
    String::from_utf16(&buffer[..end]).ok()
}

fn volume_disk_numbers(handle: HANDLE) -> io::Result<Vec<u32>> {
    let mut buffer = vec![0_u8; 4096];
    let mut returned = 0_u32;
    // SAFETY: handle is valid and buffer is writable for its advertised size.
    if unsafe {
        DeviceIoControl(
            handle,
            IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS,
            null(),
            0,
            buffer.as_mut_ptr().cast(),
            buffer.len() as u32,
            &mut returned,
            null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    if (returned as usize) < size_of::<VOLUME_DISK_EXTENTS>() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "truncated VOLUME_DISK_EXTENTS",
        ));
    }
    // SAFETY: size checked above; the returned buffer starts with VOLUME_DISK_EXTENTS.
    let extents = unsafe { &*(buffer.as_ptr().cast::<VOLUME_DISK_EXTENTS>()) };
    let count = extents.NumberOfDiskExtents as usize;
    let extent_size = size_of_val(&extents.Extents[0]);
    let required = std::mem::offset_of!(VOLUME_DISK_EXTENTS, Extents)
        .saturating_add(count.saturating_mul(extent_size));
    if required > returned as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "truncated volume disk extents",
        ));
    }
    // SAFETY: required size was validated against the returned byte count; Extents is the first
    // element of the API's variable-length trailing DISK_EXTENT array.
    let rows = unsafe { std::slice::from_raw_parts(extents.Extents.as_ptr(), count) };
    let mut numbers = rows
        .iter()
        .map(|extent| extent.DiskNumber)
        .collect::<Vec<_>>();
    numbers.sort_unstable();
    numbers.dedup();
    Ok(numbers)
}

fn disk_performance(handle: HANDLE) -> io::Result<DISK_PERFORMANCE> {
    // SAFETY: zero is a valid initial representation for this POD output structure.
    let mut output: DISK_PERFORMANCE = unsafe { zeroed() };
    let mut returned = 0_u32;
    // SAFETY: handle is valid; output points to writable DISK_PERFORMANCE storage.
    if unsafe {
        DeviceIoControl(
            handle,
            IOCTL_DISK_PERFORMANCE,
            null(),
            0,
            (&mut output as *mut DISK_PERFORMANCE).cast(),
            size_of::<DISK_PERFORMANCE>() as u32,
            &mut returned,
            null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(output)
}

pub(in crate::windows) fn storage_descriptor(handle: HANDLE) -> Option<(String, bool)> {
    let query = STORAGE_PROPERTY_QUERY {
        PropertyId: StorageDeviceProperty,
        QueryType: PropertyStandardQuery,
        AdditionalParameters: [0],
    };
    let mut buffer = vec![0_u8; 2048];
    let mut returned = 0_u32;
    // SAFETY: input/output buffers are valid for the stated sizes and handle is valid.
    if unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            (&query as *const STORAGE_PROPERTY_QUERY).cast(),
            size_of::<STORAGE_PROPERTY_QUERY>() as u32,
            buffer.as_mut_ptr().cast(),
            buffer.len() as u32,
            &mut returned,
            null_mut(),
        )
    } == 0
    {
        return None;
    }
    if (returned as usize) < size_of::<STORAGE_DEVICE_DESCRIPTOR>() {
        return None;
    }
    // SAFETY: size checked; STORAGE_DEVICE_DESCRIPTOR begins the returned buffer.
    let descriptor = unsafe { &*(buffer.as_ptr().cast::<STORAGE_DEVICE_DESCRIPTOR>()) };
    let serial = ansi_field(&buffer, descriptor.SerialNumberOffset);
    serial
        .filter(|s| !s.is_empty())
        .map(|serial| (format!("serial:{serial}"), false))
}
