use std::collections::{BTreeMap, HashMap};

use cclover_core::model::{Collection, DiskCounter, DiskId, DiskMetadata};

use super::diagnostics::{report_issue, unavailable_from_io};
use super::native;

#[derive(Clone)]
pub(super) struct DiskIdentity {
    pub(super) id: DiskId,
    pub(super) device: String,
}

pub(super) struct Batch {
    pub(super) counters: Collection<Vec<DiskCounter>>,
    pub(super) identities: HashMap<u32, DiskIdentity>,
}

pub(super) fn collect(notes: Option<&mut Vec<String>>) -> Collection<Vec<DiskCounter>> {
    collect_batch(notes).counters
}

pub(super) fn collect_batch(mut notes: Option<&mut Vec<String>>) -> Batch {
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
                Ok(result) => {
                    for failure in result.failures {
                        report_issue(&mut notes, || drive_binding_failure_note(&failure));
                    }
                    result.bindings
                }
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

            let mut identities = HashMap::new();
            let rows = result
                .disks
                .into_iter()
                .map(|disk| {
                    let id = DiskId::from_opaque_key(disk.identity);
                    let device = format!("Disk {}", disk.disk_number);
                    identities.insert(
                        disk.disk_number,
                        DiskIdentity {
                            id: id.clone(),
                            device: device.clone(),
                        },
                    );
                    DiskCounter {
                        id,
                        metadata: DiskMetadata {
                            system_label: device,
                            associated_labels: labels_by_disk
                                .remove(&disk.disk_number)
                                .unwrap_or_default(),
                        },
                        read_bytes: disk.read_bytes,
                        write_bytes: disk.write_bytes,
                    }
                })
                .collect();
            let counters = if result.fallback_identities > 0 {
                Collection::degraded(rows)
            } else {
                Collection::available(rows)
            };
            Batch {
                counters,
                identities,
            }
        }
        Err(error) => {
            report_issue(&mut notes, || {
                format!("physical disk collection failed: {error}")
            });
            Batch {
                counters: Collection::unavailable(unavailable_from_io(&error)),
                identities: HashMap::new(),
            }
        }
    }
}

fn drive_binding_failure_note(failure: &native::DriveBindingFailure) -> String {
    let action = match failure.stage {
        native::DriveBindingStage::OpenVolume => "open volume",
        native::DriveBindingStage::QueryExtents => "query volume disk extents",
    };
    format!(
        "drive-letter topology incomplete for {}: {action} failed: {}",
        failure.label, failure.error
    )
}
