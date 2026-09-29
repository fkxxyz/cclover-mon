use std::fs;
use std::path::Path;

use crate::core::model::{Collection, DiskCounter, DiskId};

use super::diagnostics::{probe_note, report_issue, unavailable_from_io};

fn disk_id(identity_path: &Path, device_number: &str) -> DiskId {
    DiskId::from_opaque_key(format!("{}#{device_number}", identity_path.display()))
}

#[cfg(any(feature = "ebpf-io", test))]
pub(super) fn id_for_name_from(block_root: &Path, name: &str) -> Option<DiskId> {
    let path = block_root.join(name);
    let device_number = fs::read_to_string(path.join("dev")).ok()?;
    let identity_path = fs::canonicalize(path.join("device"))
        .or_else(|_| fs::canonicalize(&path))
        .ok()?;
    Some(disk_id(&identity_path, device_number.trim()))
}

pub(super) fn collect(notes: Option<&mut Vec<String>>) -> Collection<Vec<DiskCounter>> {
    collect_from(Path::new("/sys/block"), notes)
}

fn collect_from(
    block_root: &Path,
    mut notes: Option<&mut Vec<String>>,
) -> Collection<Vec<DiskCounter>> {
    let mut rows = Vec::new();
    let mut degraded = false;
    let entries = match fs::read_dir(block_root) {
        Ok(entries) => entries,
        Err(error) => {
            report_issue(&mut notes, || {
                format!("cannot read {}: {error}", block_root.display())
            });
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
    use crate::platform::linux::test_support::Fixture;

    fn add_physical_disk(fixture: &Fixture, name: &str, dev: &str, stat: &str) {
        fixture.dir(format!("devices/{name}"));
        fixture.dir(format!("block/{name}"));
        fixture.symlink_to(format!("devices/{name}"), format!("block/{name}/device"));
        fixture.write(format!("block/{name}/dev"), format!("{dev}\n"));
        fixture.write(format!("block/{name}/stat"), stat);
    }

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

    #[test]
    fn discovers_physical_disks_and_skips_virtual_entries_from_fixture() {
        let fixture = Fixture::new("disk-discovery");
        fixture.dir("block");
        add_physical_disk(&fixture, "nvme0n1", "259:0", "1 2 3 4 5 6 7 8 9 10 11\n");
        fixture.write("block/loop0/stat", "1 2 99 4 5 6 88 8 9 10 11\n");

        let outcome = collect_from(&fixture.path().join("block"), None);
        let Collection::Available(rows) = outcome else {
            panic!("expected available disk collection");
        };

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "nvme0n1");
        assert_eq!(rows[0].read_bytes, 3 * 512);
        assert_eq!(rows[0].write_bytes, 7 * 512);
        assert_eq!(
            id_for_name_from(&fixture.path().join("block"), "nvme0n1"),
            Some(rows[0].id.clone())
        );
    }

    #[test]
    fn one_malformed_physical_disk_degrades_but_preserves_valid_rows() {
        let fixture = Fixture::new("disk-partial-failure");
        fixture.dir("block");
        add_physical_disk(&fixture, "nvme0n1", "259:0", "1 2 3 4 5 6 7 8 9 10 11\n");
        add_physical_disk(&fixture, "sda", "8:0", "1 2\n");

        let outcome = collect_from(&fixture.path().join("block"), None);
        let Collection::Degraded(rows) = outcome else {
            panic!("expected degraded disk collection");
        };

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "nvme0n1");
    }

    #[test]
    fn missing_block_root_is_unavailable() {
        let fixture = Fixture::new("disk-missing-root");
        assert!(matches!(
            collect_from(&fixture.path().join("missing"), None),
            Collection::Unavailable(_)
        ));
    }
}
