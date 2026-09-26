use std::collections::HashMap;
use std::fs;

use crate::core::model::MemorySnapshot;

use super::diagnostics::report_issue;

pub(super) fn collect(mut notes: Option<&mut Vec<String>>) -> Option<MemorySnapshot> {
    let text = match fs::read_to_string("/proc/meminfo") {
        Ok(text) => text,
        Err(error) => {
            report_issue(&mut notes, || format!("cannot read /proc/meminfo: {error}"));
            return None;
        }
    };

    match parse_meminfo(&text) {
        Ok(snapshot) => Some(snapshot),
        Err(error) => {
            report_issue(&mut notes, || error);
            None
        }
    }
}

fn parse_meminfo(text: &str) -> Result<MemorySnapshot, String> {
    let mut values = HashMap::new();
    for line in text.lines() {
        let Some((key, rest)) = line.split_once(':') else {
            continue;
        };
        let Some(value) = rest
            .split_whitespace()
            .next()
            .and_then(|value| value.parse::<u64>().ok())
        else {
            continue;
        };
        values.insert(key, value.saturating_mul(1024));
    }

    let Some(total_bytes) = values.get("MemTotal").copied() else {
        return Err("/proc/meminfo has no MemTotal".to_owned());
    };
    let available_bytes = values.get("MemAvailable").copied().unwrap_or(0);
    let swap_total_bytes = values.get("SwapTotal").copied().unwrap_or(0);
    let swap_free_bytes = values.get("SwapFree").copied().unwrap_or(0);

    Ok(MemorySnapshot {
        used_bytes: total_bytes.saturating_sub(available_bytes),
        total_bytes,
        swap_used_bytes: swap_total_bytes.saturating_sub(swap_free_bytes),
        swap_total_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_meminfo_without_live_procfs() {
        let text = "MemTotal:       1000 kB\nMemAvailable:    250 kB\nSwapTotal:       400 kB\nSwapFree:        100 kB\n";
        let snapshot = parse_meminfo(text).unwrap();

        assert_eq!(snapshot.total_bytes, 1000 * 1024);
        assert_eq!(snapshot.used_bytes, 750 * 1024);
        assert_eq!(snapshot.swap_total_bytes, 400 * 1024);
        assert_eq!(snapshot.swap_used_bytes, 300 * 1024);
    }
}
