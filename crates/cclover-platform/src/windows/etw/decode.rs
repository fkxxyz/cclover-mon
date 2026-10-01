use super::*;

pub(super) unsafe fn decode_event(record: &EVENT_RECORD) -> Result<Option<Event>, ()> {
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
