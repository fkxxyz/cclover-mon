use super::*;

pub(in crate::windows) fn network_interfaces() -> io::Result<Vec<NativeNetwork>> {
    let mut table: *mut MIB_IF_TABLE2 = null_mut();
    // SAFETY: table is a writable output pointer; the returned allocation is freed below.
    let status = unsafe { GetIfTable2(&mut table) };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    if table.is_null() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "GetIfTable2 returned null table",
        ));
    }
    // SAFETY: successful GetIfTable2 returns a valid table with NumEntries contiguous MIB_IF_ROW2 entries.
    let rows = unsafe {
        std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize)
    };
    let result = rows
        .iter()
        .filter(|row| {
            row.OperStatus == IfOperStatusUp
                && row.InterfaceAndOperStatusFlags._bitfield & HARDWARE_INTERFACE_FLAG != 0
        })
        .map(|row| NativeNetwork {
            guid: guid_string(row),
            name: utf16_z(&row.Alias),
            rx_bytes: row.InOctets,
            tx_bytes: row.OutOctets,
        })
        .collect();
    // SAFETY: table was allocated by GetIfTable2 and is freed exactly once.
    unsafe { FreeMibTable(table.cast()) };
    Ok(result)
}

pub(in crate::windows) fn network_identity_from_luid(luid: u64) -> io::Result<(String, String)> {
    let luid = NET_LUID_LH { Value: luid };
    let mut guid = windows_sys::core::GUID::default();
    // SAFETY: luid and guid are valid input/output objects for the documented conversion API.
    let status = unsafe { ConvertInterfaceLuidToGuid(&luid, &mut guid) };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }

    let mut alias = [0_u16; 257];
    // SAFETY: alias is writable for the supplied character count and luid is valid.
    let status = unsafe { ConvertInterfaceLuidToAlias(&luid, alias.as_mut_ptr(), alias.len()) };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }

    Ok((guid_key(&guid), utf16_z(&alias)))
}
