pub(super) fn decode_nct6701_temperature(raw: u8) -> Option<f64> {
    if raw == 0 || raw == 0xA0 || (0x7E..=0x80).contains(&raw) {
        None
    } else {
        Some(f64::from(raw as i8))
    }
}

pub(super) fn decode_13_bit(low: u8, high: u8, nct6687_zero_encoding: bool) -> Option<u64> {
    if nct6687_zero_encoding && high == 0xFF && low == 0xF8 {
        return Some(0);
    }
    let count = (u16::from(high) << 5) | u16::from(low & 0x1F);
    if count >= 0x1FFF {
        Some(0)
    } else if count < 0x15 {
        None
    } else {
        Some((1_350_000.0 / f64::from(count)).round() as u64)
    }
}
