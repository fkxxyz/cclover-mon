use std::fmt;

const UNAVAILABLE_VALUE: &str = "—";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BoundedText {
    text: String,
    max_columns: u8,
}

impl BoundedText {
    pub(crate) fn new(text: String, max_columns: u8) -> Self {
        let actual_columns = text.chars().count();
        assert!(
            actual_columns <= usize::from(max_columns),
            "bounded text exceeded its declared capacity: {actual_columns} > {max_columns}: {text:?}"
        );
        Self { text, max_columns }
    }

    pub(crate) fn empty() -> Self {
        Self::new(String::new(), 0)
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn into_string(self) -> String {
        self.text
    }

    pub const fn max_columns(&self) -> u8 {
        self.max_columns
    }

    pub fn prefixed(self, prefix: &str) -> Self {
        let prefix_columns = u8::try_from(prefix.chars().count()).unwrap_or(u8::MAX);
        let max_columns = self.max_columns.saturating_add(prefix_columns);
        Self::new(format!("{prefix}{}", self.text), max_columns)
    }
}

impl fmt::Display for BoundedText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.text)
    }
}

impl From<BoundedText> for String {
    fn from(value: BoundedText) -> Self {
        value.into_string()
    }
}

pub fn unavailable() -> String {
    UNAVAILABLE_VALUE.to_owned()
}

pub(crate) fn compact_unavailable() -> BoundedText {
    BoundedText::new(UNAVAILABLE_VALUE.to_owned(), 1)
}

pub fn format_rate(value: f64) -> String {
    format!("{}/s", format_bytes(value.max(0.0) as u64))
}

pub(crate) fn format_compact_rate(value: f64) -> BoundedText {
    format_scaled_f64(
        value,
        1024.0,
        &["B/s", "K/s", "M/s", "G/s", "T/s", "P/s", "E/s"],
        7,
    )
}

pub fn format_percent(value: f64) -> String {
    format!("{value:.1}%")
}

pub fn format_bytes(value: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut number = value as f64;
    let mut unit = 0;
    while number >= 1024.0 && unit < UNITS.len() - 1 {
        number /= 1024.0;
        unit += 1;
    }
    let formatted = if unit == 0 || number >= 100.0 {
        format!("{number:.0}")
    } else if number >= 10.0 {
        format!("{number:.1}")
    } else {
        format!("{number:.2}")
    };
    format!("{formatted} {}", UNITS[unit])
}

pub(crate) fn format_compact_bytes(value: u64) -> BoundedText {
    let unit = compact_binary_unit(value);
    let number = value as f64 / 1024_f64.powi(unit as i32);
    BoundedText::new(
        format!(
            "{}{}",
            compact_number(number, COMPACT_BINARY_NUMBER_COLUMNS),
            COMPACT_BINARY_UNITS[unit]
        ),
        COMPACT_BYTES_COLUMNS,
    )
}

pub(crate) fn format_compact_bytes_pair(used: u64, total: u64) -> BoundedText {
    let unit = compact_binary_unit(used.max(total));
    let scale = 1024_f64.powi(unit as i32);
    let used = compact_number(used as f64 / scale, COMPACT_BINARY_NUMBER_COLUMNS);
    let total = compact_number(total as f64 / scale, COMPACT_BINARY_NUMBER_COLUMNS);
    BoundedText::new(
        format!("{used}/{total} {}", COMPACT_BINARY_UNITS[unit]),
        COMPACT_BYTES_PAIR_COLUMNS,
    )
}

pub(crate) fn format_gpu_memory(used: u64, total: u64) -> BoundedText {
    let unit = compact_binary_unit(used.max(total));
    let scale = 1024_f64.powi(unit as i32);
    let used = used as f64 / scale;
    let total = total as f64 / scale;
    BoundedText::new(
        format!(
            "{used:.2} {} / {total:.0} {}",
            COMPACT_BINARY_UNITS[unit], COMPACT_BINARY_UNITS[unit]
        ),
        20,
    )
}

pub(crate) fn format_compact_percent(value: f64) -> BoundedText {
    if !value.is_finite() {
        return compact_unavailable();
    }
    let value = value.max(0.0);
    let text = if value <= 100.0 {
        format!("{value:.1}%")
    } else if value < 999.5 {
        format!("{value:.0}%")
    } else {
        "999%+".to_owned()
    };
    BoundedText::new(text, 6)
}

pub(crate) fn format_compact_temperature(value: f64) -> BoundedText {
    if !value.is_finite() {
        return compact_unavailable();
    }
    let text = if value <= -999.5 {
        "<-999°C".to_owned()
    } else if value >= 999.5 {
        ">999°C".to_owned()
    } else {
        format!("{value:.1}°C")
    };
    BoundedText::new(text, 8)
}

pub(crate) fn format_compact_power(value: f64) -> BoundedText {
    format_scaled_f64(value, 1000.0, &["W", "kW", "MW", "GW", "TW", "PW", "EW"], 7)
}

pub(crate) fn format_compact_frequency_mhz(value: u64) -> BoundedText {
    format_scaled_u64(
        value,
        1000.0,
        &["MHz", "GHz", "THz", "PHz", "EHz", "ZHz", "YHz"],
        7,
    )
}

pub(crate) fn format_compact_rpm(value: u64) -> BoundedText {
    format_scaled_u64(
        value,
        1000.0,
        &["RPM", "kRPM", "MRPM", "GRPM", "TRPM", "PRPM", "ERPM"],
        8,
    )
}

const COMPACT_BINARY_UNITS: [&str; 7] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
const COMPACT_PROMOTION_THRESHOLD: f64 = 999.5;
const COMPACT_BINARY_NUMBER_COLUMNS: u8 = 3;
const COMPACT_BINARY_UNIT_COLUMNS: u8 = 3;
const COMPACT_BYTES_COLUMNS: u8 = COMPACT_BINARY_NUMBER_COLUMNS + COMPACT_BINARY_UNIT_COLUMNS;
const COMPACT_BYTES_PAIR_COLUMNS: u8 =
    COMPACT_BINARY_NUMBER_COLUMNS * 2 + COMPACT_BINARY_UNIT_COLUMNS + 2;

fn compact_binary_unit(value: u64) -> usize {
    let mut number = value as f64;
    let mut unit = 0;
    while number >= COMPACT_PROMOTION_THRESHOLD && unit < COMPACT_BINARY_UNITS.len() - 1 {
        number /= 1024.0;
        unit += 1;
    }
    unit
}

fn compact_number(number: f64, max_columns: u8) -> String {
    let preferred_precision = usize::from(number < 10.0);
    for precision in (0..=preferred_precision).rev() {
        let text = format!("{number:.precision$}");
        if text.chars().count() <= usize::from(max_columns) {
            return text;
        }
    }
    panic!("compact number exceeded its column budget: {number} > {max_columns} columns")
}

fn compact_number_columns(max_columns: u8, suffix: &str) -> u8 {
    let suffix_columns = u8::try_from(suffix.chars().count()).unwrap_or(u8::MAX);
    max_columns
        .checked_sub(suffix_columns)
        .expect("compact suffix exceeds formatter column budget")
}

fn format_scaled_f64(value: f64, base: f64, units: &[&str], max_columns: u8) -> BoundedText {
    if !value.is_finite() {
        return compact_unavailable();
    }
    let mut number = value.max(0.0);
    let mut unit = 0;
    while number >= COMPACT_PROMOTION_THRESHOLD && unit < units.len() - 1 {
        number /= base;
        unit += 1;
    }
    let text = if unit == units.len() - 1 && number >= COMPACT_PROMOTION_THRESHOLD {
        format!(">999{}", units[unit])
    } else {
        let number_columns = compact_number_columns(max_columns, units[unit]);
        format!("{}{}", compact_number(number, number_columns), units[unit])
    };
    BoundedText::new(text, max_columns)
}

fn format_scaled_u64(value: u64, base: f64, units: &[&str], max_columns: u8) -> BoundedText {
    let mut number = value as f64;
    let mut unit = 0;
    while number >= COMPACT_PROMOTION_THRESHOLD && unit < units.len() - 1 {
        number /= base;
        unit += 1;
    }
    let text = if unit == units.len() - 1 && number >= COMPACT_PROMOTION_THRESHOLD {
        format!(">999{}", units[unit])
    } else {
        let number_columns = compact_number_columns(max_columns, units[unit]);
        format!("{}{}", compact_number(number, number_columns), units[unit])
    };
    BoundedText::new(text, max_columns)
}

#[cfg(test)]
mod tests {
    use super::compact_number;

    #[test]
    fn compact_number_uses_highest_precision_that_fits_the_budget() {
        assert_eq!(compact_number(9.94, 3), "9.9");
        assert_eq!(compact_number(9.96, 3), "10");
        assert_eq!(compact_number(9.96, 4), "10.0");
        assert_eq!(compact_number(99.6, 3), "100");
        assert_eq!(compact_number(999.4, 3), "999");
    }
}
