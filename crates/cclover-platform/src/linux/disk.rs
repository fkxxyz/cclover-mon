use std::fs;
use std::path::Path;

use cclover_core::model::{Collection, DiskCounter, DiskId, DiskMetadata};

use super::diagnostics::{probe_note, report_issue, unavailable_from_io};

fn disk_id(identity_path: &Path, device_number: &str) -> DiskId {
    DiskId::from_opaque_key(format!("{}#{device_number}", identity_path.display()))
}

/// Buses whose block devices are views of storage owned by another layer, not independent media.
/// Unknown buses are deliberately absent so that unfamiliar real disks remain visible.
const NON_PHYSICAL_BUSES: &[&str] = &["mtd", "ubi", "platform"];

/// Returns why a block device with a physical-device link is not an independent storage medium.
///
/// Both rules are structural. A block device nested under another block device (for example an
/// eMMC boot partition) shares its parent's medium. A device bound to a flash-translation or bare
/// platform bus is a firmware or flash view. Devices that match neither rule are kept.
fn non_physical_reason(block_entry: &Path) -> Option<String> {
    if let Ok(canonical) = fs::canonicalize(block_entry) {
        let parent = canonical.parent();
        let grandparent = parent.and_then(Path::parent);
        if let (Some(parent), Some(grandparent)) = (parent, grandparent)
            && parent.join("dev").is_file()
            && grandparent.file_name().is_some_and(|name| name == "block")
        {
            let owner = parent.file_name().unwrap_or_default().to_string_lossy();
            return Some(format!("nested under block device {owner}"));
        }
    }
    let subsystem = fs::canonicalize(block_entry.join("device/subsystem")).ok()?;
    let bus = subsystem.file_name()?.to_str()?;
    NON_PHYSICAL_BUSES
        .contains(&bus)
        .then(|| format!("backed by {bus} bus"))
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
        if let Some(reason) = non_physical_reason(&path) {
            probe_note(&mut notes, || format!("{name} skipped: {reason}"));
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
    rows.sort_by(|a, b| a.metadata.system_label.cmp(&b.metadata.system_label));
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
        metadata: DiskMetadata {
            system_label: name,
            associated_labels: Vec::new(),
        },
        read_bytes: read_sectors.saturating_mul(512),
        write_bytes: written_sectors.saturating_mul(512),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linux::test_support::Fixture;

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
        assert_eq!(rows[0].metadata.system_label, "nvme0n1");
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
        assert_eq!(rows[0].metadata.system_label, "nvme0n1");
    }

    fn add_block_node(fixture: &Fixture, name: &str, real: &str, owner: &str, bus: Option<&str>) {
        fixture.dir(format!("devices/{owner}"));
        if let Some(bus) = bus {
            fixture.dir(format!("bus/{bus}"));
            fixture.symlink_to(format!("bus/{bus}"), format!("devices/{owner}/subsystem"));
        }
        fixture.write(format!("{real}/dev"), "8:0\n");
        fixture.write(format!("{real}/stat"), "1 2 3 4 5 6 7 8 9 10 11\n");
        fixture.symlink_to(format!("devices/{owner}"), format!("{real}/device"));
        fixture.symlink_to(real, format!("block/{name}"));
    }

    #[test]
    fn excludes_non_physical_block_devices_and_keeps_real_media() {
        let fixture = Fixture::new("disk-non-physical");
        fixture.dir("block");
        add_block_node(
            &fixture,
            "nvme0n1",
            "devices/nvme0/block/nvme0n1",
            "nvme0",
            None,
        );
        add_block_node(
            &fixture,
            "mmcblk0",
            "devices/mmc0/block/mmcblk0",
            "mmc0",
            Some("mmc"),
        );
        add_block_node(
            &fixture,
            "mmcblk0boot0",
            "devices/mmc0/block/mmcblk0/mmcblk0boot0",
            "mmc0",
            None,
        );
        add_block_node(
            &fixture,
            "mtdblock0",
            "devices/mtd0/block/mtdblock0",
            "mtd0",
            Some("mtd"),
        );
        add_block_node(
            &fixture,
            "ubiblock0_3",
            "devices/ubi0/block/ubiblock0_3",
            "ubi0",
            Some("ubi"),
        );
        add_block_node(
            &fixture,
            "fit0",
            "devices/fitblk/block/fit0",
            "fitblk",
            Some("platform"),
        );
        add_block_node(
            &fixture,
            "sda",
            "devices/scsi/block/sda",
            "scsi",
            Some("scsi"),
        );

        let mut notes = Vec::new();
        let outcome = collect_from(&fixture.path().join("block"), Some(&mut notes));
        let Collection::Available(rows) = outcome else {
            panic!("expected available disk collection");
        };

        let names: Vec<&str> = rows
            .iter()
            .map(|row| row.metadata.system_label.as_str())
            .collect();
        assert_eq!(names, ["mmcblk0", "nvme0n1", "sda"]);
        for expected in [
            "mmcblk0boot0 skipped: nested under block device mmcblk0",
            "mtdblock0 skipped: backed by mtd bus",
            "ubiblock0_3 skipped: backed by ubi bus",
            "fit0 skipped: backed by platform bus",
        ] {
            assert!(
                notes.iter().any(|note| note == expected),
                "missing note: {expected}"
            );
        }
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
