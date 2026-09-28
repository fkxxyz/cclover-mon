use crate::core::model::{Collection, DiskCounter, DiskId};

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
            let rows = result
                .disks
                .into_iter()
                .map(|disk| DiskCounter {
                    id: DiskId::from_opaque_key(disk.identity),
                    name: disk.name,
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
