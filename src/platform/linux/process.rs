use std::ffi::OsStr;
use std::fs;
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
#[cfg(any(feature = "ebpf-io", test))]
use std::sync::OnceLock;

use crate::core::model::{Collection, ProcessCounter, ProcessInstanceId};

use super::diagnostics::{probe_note, report_issue, unavailable_from_io};
use super::native;

const PROC_PREFIX: &[u8] = b"/proc/";
const PROC_STAT_UTIME_FIELD: usize = 14;
const PROC_STAT_STIME_FIELD: usize = 15;
const PROC_STAT_STARTTIME_FIELD: usize = 22;
const PROC_STAT_RSS_FIELD: usize = 24;
const PROC_STAT_FIRST_FIELD_AFTER_COMM: usize = 3;

pub(super) struct Collector {
    page_size: u64,
    stat_buffer: String,
}

impl Collector {
    pub(super) fn new() -> Self {
        Self {
            page_size: native::page_size().unwrap_or(4096),
            stat_buffer: String::with_capacity(512),
        }
    }

    pub(super) fn collect(
        &mut self,
        mut notes: Option<&mut Vec<String>>,
    ) -> Collection<Vec<ProcessCounter>> {
        let mut processes = Vec::new();
        let entries = match fs::read_dir("/proc") {
            Ok(entries) => entries,
            Err(error) => {
                report_issue(&mut notes, || format!("cannot read /proc: {error}"));
                return Collection::unavailable(unavailable_from_io(&error));
            }
        };
        let mut unreadable = 0_u64;
        let mut malformed = 0_u64;
        let mut stat_path_buffer = Vec::with_capacity(64);
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let file_name = file_name.as_os_str().as_bytes();
            let Some(pid) = parse_pid(file_name) else {
                continue;
            };

            let stat_path = process_stat_path(&mut stat_path_buffer, file_name);
            self.stat_buffer.clear();
            let read_result = fs::File::open(stat_path)
                .and_then(|mut file| file.read_to_string(&mut self.stat_buffer));

            if read_result.is_err() {
                unreadable += 1;
                continue;
            }
            if let Some(process) = parse_stat(pid, &self.stat_buffer, self.page_size) {
                processes.push(process);
            } else {
                malformed += 1;
            }
        }
        if unreadable > 0 {
            probe_note(&mut notes, || {
                format!(
                    "{unreadable} process stat files skipped: process exited or stat was unreadable"
                )
            });
        }
        if malformed > 0 {
            probe_note(&mut notes, || {
                format!("{malformed} process stat files skipped: malformed contents")
            });
        }
        if unreadable > 0 || malformed > 0 {
            Collection::degraded(processes)
        } else {
            Collection::available(processes)
        }
    }
}

fn process_stat_path<'a>(buffer: &'a mut Vec<u8>, pid_name: &[u8]) -> &'a Path {
    buffer.clear();
    buffer.extend_from_slice(PROC_PREFIX);
    buffer.extend_from_slice(pid_name);
    buffer.extend_from_slice(b"/stat");
    Path::new(OsStr::from_bytes(buffer))
}

fn parse_pid(bytes: &[u8]) -> Option<u32> {
    if bytes.is_empty() {
        return None;
    }

    let mut pid = 0_u32;
    for byte in bytes {
        if !byte.is_ascii_digit() {
            return None;
        }
        pid = pid.checked_mul(10)?.checked_add(u32::from(*byte - b'0'))?;
    }
    Some(pid)
}

fn parse_stat(pid: u32, text: &str, page_size: u64) -> Option<ProcessCounter> {
    let left = text.find('(')?;
    let right = text.rfind(')')?;
    if right <= left {
        return None;
    }
    let (user, system, starttime, rss_pages) = parse_stat_counters(&text[right + 1..])?;
    Some(ProcessCounter {
        process: ProcessInstanceId {
            pid,
            birth_marker: starttime,
        },
        name: text[left + 1..right].to_owned(),
        cpu_time_units: user.saturating_add(system),
        rss_bytes: rss_pages.saturating_mul(page_size),
    })
}

fn parse_stat_counters(tail: &str) -> Option<(u64, u64, u64, u64)> {
    let mut fields = tail.split_ascii_whitespace();
    let user = fields
        .nth(PROC_STAT_UTIME_FIELD - PROC_STAT_FIRST_FIELD_AFTER_COMM)?
        .parse::<u64>()
        .ok()?;
    let system = fields.next()?.parse::<u64>().ok()?;
    let starttime = fields
        .nth(PROC_STAT_STARTTIME_FIELD - PROC_STAT_STIME_FIELD - 1)?
        .parse::<u64>()
        .ok()?;
    let rss_pages = fields
        .nth(PROC_STAT_RSS_FIELD - PROC_STAT_STARTTIME_FIELD - 1)?
        .parse::<u64>()
        .ok()?;

    Some((user, system, starttime, rss_pages))
}

#[cfg(any(feature = "ebpf-io", test))]
pub(super) fn birth_marker_from_start_boottime_ns(start_boottime_ns: u64) -> u64 {
    static TICKS_PER_SECOND: OnceLock<u64> = OnceLock::new();
    let ticks_per_second =
        *TICKS_PER_SECOND.get_or_init(|| native::clock_ticks_per_second().unwrap_or(100));
    let whole_seconds = start_boottime_ns / 1_000_000_000;
    let remainder_ns = start_boottime_ns % 1_000_000_000;
    whole_seconds
        .saturating_mul(ticks_per_second)
        .saturating_add(remainder_ns.saturating_mul(ticks_per_second) / 1_000_000_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_process_name_with_spaces_without_live_procfs() {
        let text =
            "42 (name with space) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22";
        let process = parse_stat(42, text, 4096).unwrap();
        assert_eq!(process.name, "name with space");
        assert_eq!(process.cpu_time_units, 23);
        assert_eq!(process.process.pid, 42);
        assert_eq!(process.process.birth_marker, 19);
    }

    #[test]
    fn parses_only_numeric_proc_entries_as_pids() {
        assert_eq!(parse_pid(b"42"), Some(42));
        assert_eq!(parse_pid(b"self"), None);
        assert_eq!(parse_pid(b""), None);
    }

    #[test]
    fn builds_process_stat_path_in_reused_buffer() {
        let mut buffer = Vec::new();
        assert_eq!(
            process_stat_path(&mut buffer, b"42"),
            Path::new("/proc/42/stat")
        );
        assert_eq!(
            process_stat_path(&mut buffer, b"7"),
            Path::new("/proc/7/stat")
        );
    }

    #[test]
    fn canonicalizes_start_boottime_to_proc_clock_ticks() {
        let ticks_per_second = native::clock_ticks_per_second().unwrap();
        assert_eq!(
            birth_marker_from_start_boottime_ns(2_500_000_000),
            2 * ticks_per_second + ticks_per_second / 2
        );
    }
}
