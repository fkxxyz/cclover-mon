use crate::core::model::{Collection, MemorySnapshot};

use super::diagnostics::{report_issue, unavailable_from_io};
use super::native;

pub(super) fn collect(mut notes: Option<&mut Vec<String>>) -> Collection<MemorySnapshot> {
    let memory = match native::memory_status() {
        Ok(memory) => memory,
        Err(error) => {
            report_issue(&mut notes, || {
                format!("GlobalMemoryStatusEx failed: {error}")
            });
            return Collection::unavailable(unavailable_from_io(&error));
        }
    };
    let page_file = match native::page_file_status() {
        Ok(page_file) => page_file,
        Err(error) => {
            report_issue(&mut notes, || format!("K32EnumPageFilesW failed: {error}"));
            return Collection::unavailable(unavailable_from_io(&error));
        }
    };

    Collection::available(MemorySnapshot {
        used_bytes: memory.total_phys.saturating_sub(memory.avail_phys),
        total_bytes: memory.total_phys,
        swap_used_bytes: page_file.used_bytes,
        swap_total_bytes: page_file.total_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::native;

    #[test]
    fn page_file_page_counts_convert_to_bytes() {
        let status = native::page_file_bytes(4096, 4096, 4094).unwrap();

        assert_eq!(status.total_bytes, 16 * 1024 * 1024);
        assert_eq!(status.used_bytes, 4094 * 4096);
    }

    #[test]
    fn page_file_byte_conversion_rejects_overflow() {
        assert!(native::page_file_bytes(u64::MAX, 2, 1).is_err());
    }
}
