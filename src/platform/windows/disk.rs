use std::collections::BTreeMap;

use crate::core::model::{Collection, DiskCounter, DiskId, DiskMetadata};

use super::diagnostics::{report_issue, unavailable_from_io};
use super::native;

pub(super) fn collect(mut notes: Option<&mut Vec<String>>) -> Collection<Vec<DiskCounter>> {
    match native::physical_disks() {
        Ok(result) => {
            if result.fallback_identities > 0 {
                report_issue(&mut notes, || {
                    format!(
                        "{} disks have no serial number; using session-stable physical-drive identity",
                        result.fallback_identities
                    )
                });
            }
            let bindings = match native::drive_bindings() {
                Ok(bindings) => bindings,
                Err(error) => {
                    report_issue(&mut notes, || {
                        format!("drive-letter topology collection failed: {error}")
                    });
                    Vec::new()
                }
            };
            let mut labels_by_disk = BTreeMap::<u32, Vec<String>>::new();
            for binding in bindings {
                for disk_number in binding.disk_numbers {
                    labels_by_disk
                        .entry(disk_number)
                        .or_default()
                        .push(binding.label.clone());
                }
            }
            for labels in labels_by_disk.values_mut() {
                labels.sort();
                labels.dedup();
            }

            let rows = result
                .disks
                .into_iter()
                .map(|disk| DiskCounter {
                    id: DiskId::from_opaque_key(disk.identity),
                    metadata: DiskMetadata {
                        system_label: format!("Disk {}", disk.disk_number),
                        associated_labels: labels_by_disk
                            .remove(&disk.disk_number)
                            .unwrap_or_default(),
                    },
                    read_bytes: disk.read_bytes,
                    write_bytes: disk.write_bytes,
                })
                .collect();
            if result.fallback_identities > 0 {
                Collection::degraded(rows)
            } else {
                Collection::available(rows)
            }
        }
        Err(error) => {
            report_issue(&mut notes, || {
                format!("physical disk collection failed: {error}")
            });
            Collection::unavailable(unavailable_from_io(&error))
        }
    }
}
