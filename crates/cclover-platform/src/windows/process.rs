use cclover_core::model::{Collection, ProcessCounter, ProcessInstanceId};

use super::diagnostics::{report_issue, unavailable_from_io};
use super::native;

pub(super) fn collect(mut notes: Option<&mut Vec<String>>) -> Collection<Vec<ProcessCounter>> {
    match native::system_processes() {
        Ok(processes) => {
            Collection::available(processes.into_iter().filter_map(process_counter).collect())
        }
        Err(error) => {
            report_issue(&mut notes, || {
                format!("NtQuerySystemInformation(SystemProcessInformation) failed: {error}")
            });
            Collection::unavailable(unavailable_from_io(&error))
        }
    }
}

fn process_counter(process: native::NativeProcess) -> Option<ProcessCounter> {
    let pid = u32::try_from(process.pid).ok()?;
    if pid == 0 {
        return None;
    }
    Some(ProcessCounter {
        process: ProcessInstanceId {
            pid,
            birth_marker: process.create_time,
        },
        name: process.name.into(),
        cpu_time_units: process.user_time.saturating_add(process.kernel_time),
        rss_bytes: process.working_set as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn native_process(pid: usize, name: &str) -> native::NativeProcess {
        native::NativeProcess {
            pid,
            name: name.to_owned(),
            create_time: 10,
            user_time: 20,
            kernel_time: 30,
            working_set: 40,
        }
    }

    #[test]
    fn excludes_idle_pseudo_process_but_keeps_system_process() {
        assert!(process_counter(native_process(0, "System Idle Process")).is_none());

        let system = process_counter(native_process(4, "System")).expect("system process");
        assert_eq!(system.process.pid, 4);
        assert_eq!(system.name.as_ref(), "System");
    }
}
