use crate::core::model::{Collection, CpuCounter};

use super::diagnostics::{report_issue, unavailable_from_io};
use super::native;

pub(super) fn collect(mut notes: Option<&mut Vec<String>>) -> Collection<CpuCounter> {
    match native::system_cpu_times() {
        Ok(times) => Collection::available(CpuCounter {
            total_time_units: times.kernel.saturating_add(times.user),
            idle_time_units: times.idle,
            logical_cpu_count: times.logical_cpu_count.max(1),
        }),
        Err(error) => {
            report_issue(&mut notes, || format!("GetSystemTimes failed: {error}"));
            Collection::unavailable(unavailable_from_io(&error))
        }
    }
}
