use crate::core::model::{Collection, MemorySnapshot};

use super::diagnostics::{report_issue, unavailable_from_io};
use super::native;

pub(super) fn collect(mut notes: Option<&mut Vec<String>>) -> Collection<MemorySnapshot> {
    match native::memory_status() {
        Ok(memory) => Collection::available(MemorySnapshot {
            used_bytes: memory.total_phys.saturating_sub(memory.avail_phys),
            total_bytes: memory.total_phys,
            swap_used_bytes: memory
                .total_page_file
                .saturating_sub(memory.avail_page_file),
            swap_total_bytes: memory.total_page_file,
        }),
        Err(error) => {
            report_issue(&mut notes, || {
                format!("GlobalMemoryStatusEx failed: {error}")
            });
            Collection::unavailable(unavailable_from_io(&error))
        }
    }
}
