use std::fs;
use std::path::Path;

use crate::core::model::{Collection, CollectionUnavailable, CpuCounter};

use super::diagnostics::{report_issue, unavailable_from_io};

pub(super) fn collect(notes: Option<&mut Vec<String>>) -> Collection<CpuCounter> {
    collect_from(Path::new("/proc/stat"), notes)
}

fn collect_from(path: &Path, mut notes: Option<&mut Vec<String>>) -> Collection<CpuCounter> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            report_issue(&mut notes, || {
                format!("cannot read {}: {error}", path.display())
            });
            return Collection::unavailable(unavailable_from_io(&error));
        }
    };

    match parse_stat(&text) {
        Ok(counter) => Collection::available(counter),
        Err(error) => {
            report_issue(&mut notes, || error);
            Collection::unavailable(CollectionUnavailable::InvalidData)
        }
    }
}

fn parse_stat(text: &str) -> Result<CpuCounter, String> {
    let mut lines = text.lines();
    let Some(first) = lines.next() else {
        return Err("/proc/stat is empty".to_owned());
    };
    let mut fields = first.split_whitespace();
    if fields.next() != Some("cpu") {
        return Err("/proc/stat has no aggregate cpu row".to_owned());
    }
    let values: Vec<u64> = fields.filter_map(|value| value.parse().ok()).collect();
    if values.len() < 5 {
        return Err(format!(
            "/proc/stat aggregate cpu row has only {} counters",
            values.len()
        ));
    }
    let total_time_units = values.iter().copied().sum();
    let idle_time_units = values.get(3).copied().unwrap_or(0) + values.get(4).copied().unwrap_or(0);
    let logical_cpu_count = lines
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| {
            name.strip_prefix("cpu").is_some_and(|suffix| {
                !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
            })
        })
        .count()
        .max(1);

    Ok(CpuCounter {
        total_time_units,
        idle_time_units,
        logical_cpu_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::model::CollectionUnavailable;
    use crate::platform::linux::test_support::Fixture;

    #[test]
    fn parses_proc_stat_without_live_procfs() {
        let text = "cpu  10 2 3 20 5 1 1 0 0 0\ncpu0 1 0 0 1 0 0 0 0 0 0\ncpu1 1 0 0 1 0 0 0 0 0 0\nintr 0\n";
        let counter = parse_stat(text).unwrap();

        assert_eq!(counter.total_time_units, 42);
        assert_eq!(counter.idle_time_units, 25);
        assert_eq!(counter.logical_cpu_count, 2);
    }

    #[test]
    fn collects_cpu_from_fixture_path() {
        let fixture = Fixture::new("cpu");
        fixture.write(
            "stat",
            "cpu  10 2 3 20 5 1 1 0 0 0\ncpu0 1 0 0 1 0 0 0 0 0 0\n",
        );

        let Collection::Available(counter) = collect_from(&fixture.path().join("stat"), None)
        else {
            panic!("expected available CPU collection");
        };
        assert_eq!(counter.total_time_units, 42);
        assert_eq!(counter.logical_cpu_count, 1);
    }

    #[test]
    fn malformed_cpu_fixture_is_invalid_data() {
        let fixture = Fixture::new("cpu-invalid");
        fixture.write("stat", "not-cpu 1 2 3 4 5\n");

        assert!(matches!(
            collect_from(&fixture.path().join("stat"), None),
            Collection::Unavailable(CollectionUnavailable::InvalidData)
        ));
    }

    #[test]
    fn missing_cpu_fixture_is_unavailable() {
        let fixture = Fixture::new("cpu-missing");
        assert!(matches!(
            collect_from(&fixture.path().join("missing"), None),
            Collection::Unavailable(CollectionUnavailable::Unavailable)
        ));
    }
}
