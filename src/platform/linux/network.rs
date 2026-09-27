use std::fs;
use std::path::Path;

use crate::core::model::{NetworkCounter, NetworkId};

use super::diagnostics::{probe_note, report_issue};

pub(super) fn collect(mut notes: Option<&mut Vec<String>>) -> Vec<NetworkCounter> {
    let mut rows = Vec::new();
    let entries = match fs::read_dir("/sys/class/net") {
        Ok(entries) => entries,
        Err(error) => {
            report_issue(&mut notes, || {
                format!("cannot read /sys/class/net: {error}")
            });
            return rows;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if !path.join("device").exists() {
            probe_note(&mut notes, || {
                format!("{name} skipped: no physical device link")
            });
            continue;
        }
        let Some(ifindex) = read_u64(path.join("ifindex")) else {
            probe_note(&mut notes, || {
                format!("{name} skipped: ifindex is unreadable")
            });
            continue;
        };
        let Some(device_path) = fs::canonicalize(path.join("device")).ok() else {
            probe_note(&mut notes, || {
                format!("{name} skipped: device identity is unreadable")
            });
            continue;
        };
        let operstate = read_trimmed(path.join("operstate"));
        if operstate.as_deref() != Some("up") {
            probe_note(&mut notes, || {
                format!(
                    "{name} skipped: operstate is {}",
                    operstate.as_deref().unwrap_or("unreadable")
                )
            });
            continue;
        }
        let Some(received_bytes) = read_u64(path.join("statistics/rx_bytes")) else {
            probe_note(&mut notes, || {
                format!("{name} skipped: rx_bytes is unreadable")
            });
            continue;
        };
        let Some(transmitted_bytes) = read_u64(path.join("statistics/tx_bytes")) else {
            probe_note(&mut notes, || {
                format!("{name} skipped: tx_bytes is unreadable")
            });
            continue;
        };
        rows.push(NetworkCounter {
            id: NetworkId::from_opaque_key(format!("{}#{ifindex}", device_path.display())),
            name,
            rx_bytes: received_bytes,
            tx_bytes: transmitted_bytes,
        });
    }
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    rows
}

fn read_u64(path: impl AsRef<Path>) -> Option<u64> {
    parse_u64(&fs::read_to_string(path).ok()?)
}

fn read_trimmed(path: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|text| text.trim().to_owned())
}

fn parse_u64(text: &str) -> Option<u64> {
    text.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sysfs_counter_without_live_sysfs() {
        assert_eq!(parse_u64("12345\n"), Some(12345));
        assert_eq!(parse_u64("not-a-counter\n"), None);
    }
}
