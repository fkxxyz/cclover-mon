use std::fs;
use std::path::Path;
#[cfg(feature = "ebpf-io")]
use std::path::PathBuf;

use crate::core::model::{Collection, DiskCounter, DiskId};

use super::diagnostics::{probe_note, report_issue, unavailable_from_io};

fn disk_id(identity_path: &Path, device_number: &str) -> DiskId {
    DiskId::from_opaque_key(format!("{}#{device_number}", identity_path.display()))
}

#[cfg(feature = "ebpf-io")]
pub(super) fn id_for_name(name: &str) -> Option<DiskId> {
    let path = PathBuf::from("/sys/block").join(name);
    let device_number = fs::read_to_string(path.join("dev")).ok()?;
    let identity_path = fs::canonicalize(path.join("device"))
        .or_else(|_| fs::canonicalize(&path))
        .ok()?;
    Some(disk_id(&identity_path, device_number.trim()))
}

pub(super) fn collect(mut notes: Option<&mut Vec<String>>) -> Collection<Vec<DiskCounter>> {
    let mut rows = Vec::new();
    let mut degraded = false;
    let entries = match fs::read_dir("/sys/block") {
        Ok(entries) => entries,
        Err(error) => {
            report_issue(&mut notes, || format!("cannot read /sys/block: {error}"));
            return Collection::unavailable(unavailable_from_io(&error));
        }
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        if !path.join("device").exists() {
            probe_note(&mut notes, || {
                format!("{name} skipped: no physical device link")
            });
            continue;
        }
        let Some(device_path) = fs::canonicalize(path.join("device")).ok() else {
            degraded = true;
            probe_note(&mut notes, || {
                format!("{name} skipped: device identity is unreadable")
            });
            continue;
        };
        let Some(device_number) = fs::read_to_string(path.join("dev"))
            .ok()
            .map(|value| value.trim().to_owned())
        else {
            degraded = true;
            probe_note(&mut notes, || {
                format!("{name} skipped: device number is unreadable")
            });
            continue;
        };
        let stat = match fs::read_to_string(path.join("stat")) {
            Ok(stat) => stat,
            Err(_) => {
                degraded = true;
                probe_note(&mut notes, || format!("{name} skipped: stat is unreadable"));
                continue;
            }
        };
        let id = disk_id(&device_path, &device_number);
        match parse_stat(id, name.clone(), &stat) {
            Ok(counter) => rows.push(counter),
            Err(field_count) => {
                degraded = true;
                probe_note(&mut notes, || {
                    format!("{name} skipped: stat has only {field_count} fields")
                });
            }
        }
    }
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    if degraded {
        Collection::degraded(rows)
    } else {
        Collection::available(rows)
    }
}

fn parse_stat(id: DiskId, name: String, text: &str) -> Result<DiskCounter, usize> {
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
        id,
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
        let counter = parse_stat(
            DiskId::from_opaque_key("disk-a"),
            "nvme0n1".to_owned(),
            "1 2 3 4 5 6 7 8 9 10 11",
        )
        .unwrap();

        assert_eq!(counter.read_bytes, 3 * 512);
        assert_eq!(counter.write_bytes, 7 * 512);
    }
}
