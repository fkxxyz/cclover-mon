use std::fs;

use crate::core::model::DiskCounter;

use super::diagnostics::{probe_note, report_issue};

pub(super) fn collect(mut notes: Option<&mut Vec<String>>) -> Vec<DiskCounter> {
    let mut rows = Vec::new();
    let entries = match fs::read_dir("/sys/block") {
        Ok(entries) => entries,
        Err(error) => {
            report_issue(&mut notes, || format!("cannot read /sys/block: {error}"));
            return rows;
        }
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !entry.path().join("device").exists() {
            probe_note(&mut notes, || {
                format!("{name} skipped: no physical device link")
            });
            continue;
        }
        let stat = match fs::read_to_string(entry.path().join("stat")) {
            Ok(stat) => stat,
            Err(_) => {
                probe_note(&mut notes, || format!("{name} skipped: stat is unreadable"));
                continue;
            }
        };
        match parse_stat(name.clone(), &stat) {
            Ok(counter) => rows.push(counter),
            Err(field_count) => {
                probe_note(&mut notes, || {
                    format!("{name} skipped: stat has only {field_count} fields")
                });
            }
        }
    }
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    rows
}

fn parse_stat(name: String, text: &str) -> Result<DiskCounter, usize> {
    let fields: Vec<&str> = text.split_whitespace().collect();
    if fields.len() < 7 {
        return Err(fields.len());
    }
    let read_sectors = fields
        .get(2)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    let written_sectors = fields
        .get(6)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);

    Ok(DiskCounter {
        name,
        read_bytes: read_sectors.saturating_mul(512),
        write_bytes: written_sectors.saturating_mul(512),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_block_stat_without_live_sysfs() {
        let counter = parse_stat("nvme0n1".to_owned(), "1 2 3 4 5 6 7 8 9 10 11").unwrap();

        assert_eq!(counter.read_bytes, 3 * 512);
        assert_eq!(counter.write_bytes, 7 * 512);
    }
}
