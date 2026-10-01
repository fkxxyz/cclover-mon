use super::*;

pub(in crate::windows) fn system_processes() -> io::Result<Vec<NativeProcess>> {
    let mut buffer = vec![0_u8; 256 * 1024];
    loop {
        let mut required = 0_u32;
        // SAFETY: buffer is writable for its advertised length; return-length points to writable u32.
        let status = unsafe {
            NtQuerySystemInformation(
                SystemProcessInformation,
                buffer.as_mut_ptr().cast(),
                buffer.len() as u32,
                &mut required,
            )
        };
        if status == STATUS_INFO_LENGTH_MISMATCH {
            buffer.resize((required as usize).saturating_add(64 * 1024), 0);
            continue;
        }
        if status < 0 {
            return Err(io::Error::other(format!(
                "NTSTATUS 0x{:08x}",
                status as u32
            )));
        }
        break;
    }
    let mut result = Vec::new();
    let mut offset = 0_usize;
    loop {
        if offset + size_of::<SYSTEM_PROCESS_INFORMATION>() > buffer.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "truncated SYSTEM_PROCESS_INFORMATION",
            ));
        }
        // SAFETY: bounds checked above; Windows aligns every entry suitably for this structure.
        let row = unsafe {
            &*(buffer
                .as_ptr()
                .add(offset)
                .cast::<SYSTEM_PROCESS_INFORMATION>())
        };
        let reserved = &row.Reserved1;
        let create_time = i64_field(reserved, 24).max(0) as u64;
        let user_time = i64_field(reserved, 32).max(0) as u64;
        let kernel_time = i64_field(reserved, 40).max(0) as u64;
        let pid = row.UniqueProcessId as usize;
        let name = unicode_string(&row.ImageName).unwrap_or_else(|| {
            if pid == 0 {
                "System Idle Process".to_owned()
            } else {
                format!("pid-{pid}")
            }
        });
        result.push(NativeProcess {
            pid,
            name,
            create_time,
            user_time,
            kernel_time,
            working_set: row.WorkingSetSize,
        });
        if row.NextEntryOffset == 0 {
            break;
        }
        offset = offset
            .checked_add(row.NextEntryOffset as usize)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "process entry offset overflow")
            })?;
    }
    Ok(result)
}
