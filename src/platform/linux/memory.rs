use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::core::model::{Collection, CollectionUnavailable, MemorySnapshot};

use super::diagnostics::{report_issue, unavailable_from_io};

pub(super) fn collect(notes: Option<&mut Vec<String>>) -> Collection<MemorySnapshot> {
    collect_from(Path::new("/proc/meminfo"), notes)
}

fn collect_from(path: &Path, mut notes: Option<&mut Vec<String>>) -> Collection<MemorySnapshot> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            report_issue(&mut notes, || {
                format!("cannot read {}: {error}", path.display())
            });
            return Collection::unavailable(unavailable_from_io(&error));
        }
    };

    match parse_meminfo(&text) {
        Ok(snapshot) => Collection::available(snapshot),
        Err(error) => {
            report_issue(&mut notes, || error);
            Collection::unavailable(CollectionUnavailable::InvalidData)
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
    use crate::core::model::CollectionUnavailable;
    use crate::platform::linux::test_support::Fixture;

    #[test]
    fn parses_meminfo_without_live_procfs() {
        let text = "MemTotal:       1000 kB\nMemAvailable:    250 kB\nSwapTotal:       400 kB\nSwapFree:        100 kB\n";
        let snapshot = parse_meminfo(text).unwrap();

        assert_eq!(snapshot.total_bytes, 1000 * 1024);
        assert_eq!(snapshot.used_bytes, 750 * 1024);
        assert_eq!(snapshot.swap_total_bytes, 400 * 1024);
        assert_eq!(snapshot.swap_used_bytes, 300 * 1024);
    }

    #[test]
    fn collects_memory_from_fixture_path() {
        let fixture = Fixture::new("memory");
        fixture.write(
            "meminfo",
            "MemTotal: 1000 kB\nMemAvailable: 250 kB\nSwapTotal: 400 kB\nSwapFree: 100 kB\n",
        );

        let Collection::Available(snapshot) = collect_from(&fixture.path().join("meminfo"), None)
        else {
            panic!("expected available memory collection");
        };
        assert_eq!(snapshot.used_bytes, 750 * 1024);
        assert_eq!(snapshot.swap_used_bytes, 300 * 1024);
    }

    #[test]
    fn malformed_memory_fixture_is_invalid_data() {
        let fixture = Fixture::new("memory-invalid");
        fixture.write("meminfo", "MemAvailable: 250 kB\n");

        assert!(matches!(
            collect_from(&fixture.path().join("meminfo"), None),
            Collection::Unavailable(CollectionUnavailable::InvalidData)
        ));
    }

    #[test]
    fn missing_memory_fixture_is_unavailable() {
        let fixture = Fixture::new("memory-missing");
        assert!(matches!(
            collect_from(&fixture.path().join("missing"), None),
            Collection::Unavailable(CollectionUnavailable::Unavailable)
        ));
    }
}
