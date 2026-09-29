use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::FileExt;
use std::path::Path;
use std::sync::Arc;
#[cfg(any(feature = "ebpf-io", test))]
use std::sync::OnceLock;

use crate::core::model::{Collection, ProcessCounter, ProcessInstanceId};

use super::diagnostics::{probe_note, report_issue, unavailable_from_io};
use super::native;

const PROC_STAT_UTIME_FIELD: usize = 14;
const PROC_STAT_STIME_FIELD: usize = 15;
const PROC_STAT_STARTTIME_FIELD: usize = 22;
const PROC_STAT_RSS_FIELD: usize = 24;
const PROC_STAT_FIRST_FIELD_AFTER_COMM: usize = 3;

pub(super) struct Collector {
    page_size: u64,
    stat_buffer: [u8; 4096],
    stat_fallback: String,
    stat_files: HashMap<u32, CachedStat>,
    generation: u64,
}

struct CachedStat {
    file: fs::File,
    proc_inode: u64,
    generation: u64,
    name: Option<Arc<str>>,
}

impl Collector {
    pub(super) fn new() -> Self {
        Self {
            page_size: native::page_size().unwrap_or(4096),
            stat_buffer: [0; 4096],
            stat_fallback: String::with_capacity(4096),
            stat_files: HashMap::new(),
            generation: 0,
        }
    }

    pub(super) fn collect(
        &mut self,
        notes: Option<&mut Vec<String>>,
    ) -> Collection<Vec<ProcessCounter>> {
        self.collect_from(Path::new("/proc"), notes)
    }

    fn collect_from(
        &mut self,
        proc_root: &Path,
        mut notes: Option<&mut Vec<String>>,
    ) -> Collection<Vec<ProcessCounter>> {
        let mut processes = Vec::new();
        let entries = match native::numeric_directory_entries(proc_root) {
            Ok(entries) => entries,
            Err(error) => {
                report_issue(&mut notes, || {
                    format!("cannot read {}: {error}", proc_root.display())
                });
                return Collection::unavailable(unavailable_from_io(&error));
            }
        };
        let mut unreadable = 0_u64;
        let mut malformed = 0_u64;
        let mut stat_path_buffer = Vec::with_capacity(64);
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        for (pid, proc_inode) in entries {
            let stat_path = process_stat_path(&mut stat_path_buffer, proc_root, pid);
            if self
                .stat_files
                .get(&pid)
                .is_some_and(|cached| cached.proc_inode != proc_inode)
            {
                self.stat_files.remove(&pid);
            }
            if let std::collections::hash_map::Entry::Vacant(entry) = self.stat_files.entry(pid) {
                let Ok(file) = fs::File::open(stat_path) else {
                    unreadable += 1;
                    continue;
                };
                entry.insert(CachedStat {
                    file,
                    proc_inode,
                    generation,
                    name: None,
                });
            }
            let read_result = self
                .stat_files
                .get_mut(&pid)
                .map(|cached| {
                    cached.generation = generation;
                    cached.file.read_at(&mut self.stat_buffer, 0)
                })
                .expect("process stat cache entry must exist");
            let read_result = match read_result {
                Ok(bytes) => Ok(bytes),
                Err(_) => {
                    self.stat_files.remove(&pid);
                    match fs::File::open(stat_path) {
                        Ok(file) => {
                            let result = file.read_at(&mut self.stat_buffer, 0);
                            self.stat_files.insert(
                                pid,
                                CachedStat {
                                    file,
                                    proc_inode,
                                    generation,
                                    name: None,
                                },
                            );
                            result
                        }
                        Err(error) => Err(error),
                    }
                }
            };
            let text = match read_result {
                Ok(bytes) if bytes < self.stat_buffer.len() => {
                    std::str::from_utf8(&self.stat_buffer[..bytes]).ok()
                }
                Ok(_) => {
                    self.stat_fallback.clear();
                    self.stat_files.get_mut(&pid).and_then(|cached| {
                        cached
                            .file
                            .read_to_string(&mut self.stat_fallback)
                            .ok()
                            .map(|_| self.stat_fallback.as_str())
                    })
                }
                Err(_) => None,
            };

            let Some(text) = text else {
                unreadable += 1;
                continue;
            };
            let cached_name = self
                .stat_files
                .get(&pid)
                .and_then(|cached| cached.name.as_ref());
            if let Some(process) =
                parse_stat_with_cached_name(pid, text, self.page_size, cached_name)
            {
                if let Some(cached) = self.stat_files.get_mut(&pid) {
                    cached.name = Some(Arc::clone(&process.name));
                }
                processes.push(process);
            } else {
                malformed += 1;
            }
        }
        self.stat_files
            .retain(|_, cached| cached.generation == generation);
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

fn process_stat_path<'a>(buffer: &'a mut Vec<u8>, proc_root: &Path, pid: u32) -> &'a Path {
    buffer.clear();
    buffer.extend_from_slice(proc_root.as_os_str().as_bytes());
    if !buffer.ends_with(b"/") {
        buffer.push(b'/');
    }
    append_u32_decimal(buffer, pid);
    buffer.extend_from_slice(b"/stat");
    Path::new(OsStr::from_bytes(buffer))
}

fn append_u32_decimal(buffer: &mut Vec<u8>, mut value: u32) {
    let start = buffer.len();
    if value == 0 {
        buffer.push(b'0');
        return;
    }
    while value > 0 {
        buffer.push(b'0' + (value % 10) as u8);
        value /= 10;
    }
    buffer[start..].reverse();
}

#[cfg(test)]
fn parse_stat(pid: u32, text: &str, page_size: u64) -> Option<ProcessCounter> {
    parse_stat_with_cached_name(pid, text, page_size, None)
}

fn parse_stat_with_cached_name(
    pid: u32,
    text: &str,
    page_size: u64,
    cached_name: Option<&Arc<str>>,
) -> Option<ProcessCounter> {
    let left = text.find('(')?;
    let right = text.rfind(')')?;
    if right <= left {
        return None;
    }
    let (user, system, starttime, rss_pages) = parse_stat_counters(&text[right + 1..])?;
    let name = &text[left + 1..right];
    Some(ProcessCounter {
        process: ProcessInstanceId {
            pid,
            birth_marker: starttime,
        },
        name: cached_name
            .filter(|cached| cached.as_ref() == name)
            .map(Arc::clone)
            .unwrap_or_else(|| Arc::from(name)),
        cpu_time_units: user.saturating_add(system),
        rss_bytes: rss_pages.saturating_mul(page_size),
    })
}

fn parse_stat_counters(tail: &str) -> Option<(u64, u64, u64, u64)> {
    let bytes = tail.as_bytes();
    let mut cursor = 0_usize;
    let mut field = PROC_STAT_FIRST_FIELD_AFTER_COMM;
    let mut user = None;
    let mut system = None;
    let mut starttime = None;
    let mut rss_pages = None;

    while field <= PROC_STAT_RSS_FIELD {
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        let start = cursor;
        while cursor < bytes.len() && !bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if start == cursor {
            return None;
        }

        match field {
            PROC_STAT_UTIME_FIELD => user = parse_u64_decimal(&bytes[start..cursor]),
            PROC_STAT_STIME_FIELD => system = parse_u64_decimal(&bytes[start..cursor]),
            PROC_STAT_STARTTIME_FIELD => starttime = parse_u64_decimal(&bytes[start..cursor]),
            PROC_STAT_RSS_FIELD => rss_pages = parse_u64_decimal(&bytes[start..cursor]),
            _ => {}
        }
        field += 1;
    }

    Some((user?, system?, starttime?, rss_pages?))
}

fn parse_u64_decimal(bytes: &[u8]) -> Option<u64> {
    if bytes.is_empty() {
        return None;
    }
    let mut value = 0_u64;
    for byte in bytes {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value
            .checked_mul(10)?
            .checked_add(u64::from(*byte - b'0'))?;
    }
    Some(value)
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
    use crate::platform::linux::test_support::Fixture;

    #[test]
    fn parses_process_name_with_spaces_without_live_procfs() {
        let text =
            "42 (name with space) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22";
        let process = parse_stat(42, text, 4096).unwrap();
        assert_eq!(process.name.as_ref(), "name with space");
        assert_eq!(process.cpu_time_units, 23);
        assert_eq!(process.process.pid, 42);
        assert_eq!(process.process.birth_marker, 19);
    }

    #[test]
    fn builds_process_stat_path_in_reused_buffer() {
        let mut buffer = Vec::new();
        assert_eq!(
            process_stat_path(&mut buffer, Path::new("/proc"), 42),
            Path::new("/proc/42/stat")
        );
        assert_eq!(
            process_stat_path(&mut buffer, Path::new("/proc"), 7),
            Path::new("/proc/7/stat")
        );
    }

    #[test]
    fn discovers_processes_from_fixture_and_reports_partial_failure() {
        let fixture = Fixture::new("process-discovery");
        fixture.write(
            "42/stat",
            "42 (fixture process) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22",
        );
        fixture.dir("7");
        fixture.write("self/stat", "not a numeric directory");

        let mut collector = Collector::new();
        let outcome = collector.collect_from(fixture.path(), None);
        let Collection::Degraded(processes) = outcome else {
            panic!("expected degraded process collection");
        };

        assert_eq!(processes.len(), 1);
        assert_eq!(processes[0].process.pid, 42);
        assert_eq!(processes[0].name.as_ref(), "fixture process");
    }

    #[test]
    fn reopens_cached_stat_when_pid_directory_identity_changes() {
        let fixture = Fixture::new("process-pid-reuse");
        fixture.write(
            "42/stat",
            "42 (old process) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22",
        );
        let old_inode = native::numeric_directory_entries(fixture.path())
            .unwrap()
            .into_iter()
            .find(|(pid, _)| *pid == 42)
            .unwrap()
            .1;

        let mut collector = Collector::new();
        let first = collector.collect_from(fixture.path(), None);
        let first = first.value().unwrap();
        assert_eq!(first[0].name.as_ref(), "old process");

        fs::remove_dir_all(fixture.path().join("42")).unwrap();
        fixture.dir("43");
        fixture.write(
            "42/stat",
            "42 (new process) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 99 20 21 22",
        );
        let new_inode = native::numeric_directory_entries(fixture.path())
            .unwrap()
            .into_iter()
            .find(|(pid, _)| *pid == 42)
            .unwrap()
            .1;
        assert_ne!(old_inode, new_inode, "fixture must model a new proc inode");

        let second = collector.collect_from(fixture.path(), None);
        let second = second.value().unwrap();
        let process = second
            .iter()
            .find(|process| process.process.pid == 42)
            .unwrap();
        assert_eq!(process.name.as_ref(), "new process");
        assert_ne!(process.process.birth_marker, first[0].process.birth_marker);
        assert_eq!(collector.stat_files.get(&42).unwrap().proc_inode, new_inode);
    }

    #[test]
    fn missing_proc_root_is_unavailable() {
        let fixture = Fixture::new("process-missing-root");
        let mut collector = Collector::new();
        assert!(matches!(
            collector.collect_from(&fixture.path().join("missing"), None),
            Collection::Unavailable(_)
        ));
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
