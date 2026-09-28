use crate::core::model::{Collection, ProcessCounter, ProcessInstanceId};

use super::diagnostics::{report_issue, unavailable_from_io};
use super::native;

pub(super) fn collect(mut notes: Option<&mut Vec<String>>) -> Collection<Vec<ProcessCounter>> {
    match native::system_processes() {
        Ok(processes) => Collection::available(
            processes
                .into_iter()
                .filter_map(|process| {
                    let pid = u32::try_from(process.pid).ok()?;
                    Some(ProcessCounter {
                        process: ProcessInstanceId {
                            pid,
                            birth_marker: process.create_time,
                        },
                        name: process.name,
                        cpu_time_units: process.user_time.saturating_add(process.kernel_time),
                        rss_bytes: process.working_set as u64,
                    })
                })
                .collect(),
        ),
        Err(error) => {
            report_issue(&mut notes, || {
                format!("NtQuerySystemInformation(SystemProcessInformation) failed: {error}")
            });
            Collection::unavailable(unavailable_from_io(&error))
        }
    }
}
