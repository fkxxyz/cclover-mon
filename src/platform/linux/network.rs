use std::fs;
use std::path::Path;

use crate::core::model::{Collection, NetworkCounter, NetworkId};

use super::diagnostics::{probe_note, report_issue, unavailable_from_io};

fn network_id(identity_path: &Path, ifindex: u64) -> NetworkId {
    NetworkId::from_opaque_key(format!("{}#{ifindex}", identity_path.display()))
}

pub(super) fn id_for_interface(name: &str, ifindex: u32) -> Option<NetworkId> {
    let path = Path::new("/sys/class/net").join(name);
    let identity_path = fs::canonicalize(path.join("device"))
        .or_else(|_| fs::canonicalize(&path))
        .ok()?;
    Some(network_id(&identity_path, u64::from(ifindex)))
}

pub(super) fn collect(mut notes: Option<&mut Vec<String>>) -> Collection<Vec<NetworkCounter>> {
    let mut rows = Vec::new();
    let mut degraded = false;
    let entries = match fs::read_dir("/sys/class/net") {
        Ok(entries) => entries,
        Err(error) => {
            report_issue(&mut notes, || {
                format!("cannot read /sys/class/net: {error}")
            });
            return Collection::unavailable(unavailable_from_io(&error));
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
            degraded = true;
            probe_note(&mut notes, || {
                format!("{name} skipped: ifindex is unreadable")
            });
            continue;
        };
        let Some(device_path) = fs::canonicalize(path.join("device")).ok() else {
            degraded = true;
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
            degraded = true;
            probe_note(&mut notes, || {
                format!("{name} skipped: rx_bytes is unreadable")
            });
            continue;
        };
        let Some(transmitted_bytes) = read_u64(path.join("statistics/tx_bytes")) else {
            degraded = true;
            probe_note(&mut notes, || {
                format!("{name} skipped: tx_bytes is unreadable")
            });
            continue;
        };
        rows.push(NetworkCounter {
            id: network_id(&device_path, ifindex),
            name,
            rx_bytes: received_bytes,
            tx_bytes: transmitted_bytes,
        });
    }
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    if degraded {
        Collection::degraded(rows)
    } else {
        Collection::available(rows)
    }
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
