use std::fs;
use std::io::Read;
use std::path::PathBuf;

use crate::core::model::ProcessCounter;

use super::diagnostics::{probe_note, report_issue};

pub(super) struct Collector {
    page_size: u64,
    stat_buffer: String,
}

impl Collector {
    pub(super) fn new() -> Self {
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        Self {
            page_size: if page_size > 0 {
                page_size as u64
            } else {
                4096
            },
            stat_buffer: String::with_capacity(512),
        }
    }

    pub(super) fn collect(&mut self, mut notes: Option<&mut Vec<String>>) -> Vec<ProcessCounter> {
        let mut processes = Vec::new();
        let entries = match fs::read_dir("/proc") {
            Ok(entries) => entries,
            Err(error) => {
                report_issue(&mut notes, || format!("cannot read /proc: {error}"));
                return processes;
            }
        };
        let mut unreadable = 0_u64;
        let mut malformed = 0_u64;
        let mut stat_path = PathBuf::with_capacity(64);
        stat_path.push("/proc");
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let Some(pid) = file_name.to_str().and_then(|name| name.parse::<u32>().ok()) else {
                continue;
            };

            stat_path.push(&file_name);
            stat_path.push("stat");
            self.stat_buffer.clear();
            let read_result = fs::File::open(&stat_path)
                .and_then(|mut file| file.read_to_string(&mut self.stat_buffer));
            stat_path.pop();
            stat_path.pop();

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
        processes
    }
}

fn parse_stat(pid: u32, text: &str, page_size: u64) -> Option<ProcessCounter> {
    let left = text.find('(')?;
    let right = text.rfind(')')?;
    if right <= left {
        return None;
    }
    let mut fields = text[right + 1..].split_ascii_whitespace();
    let user = fields.nth(11)?.parse::<u64>().ok()?;
    let system = fields.next()?.parse::<u64>().ok()?;
    let rss_pages = fields.nth(8)?.parse::<u64>().ok()?;
    Some(ProcessCounter {
        pid,
        name: text[left + 1..right].to_owned(),
        cpu_ticks: user.saturating_add(system),
        rss_bytes: rss_pages.saturating_mul(page_size),
    })
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
        assert_eq!(process.cpu_ticks, 23);
    }
}
