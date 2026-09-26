use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::core::devlog;
use crate::core::model::{
    CpuCounter, DiskCounter, MemorySnapshot, NetworkCounter, ProcessCounter, RawSnapshot,
    TemperatureSnapshot,
};
use crate::platform::{Collector, ProbeKind, ProbeReport};

const TEMPERATURE_INTERVAL: Duration = Duration::from_secs(2);
const MAX_SENSOR_BACKOFF: Duration = Duration::from_secs(300);

#[derive(Debug, Clone)]
struct SensorRetry {
    failures: u32,
    retry_at: Instant,
}

pub struct Backend {
    page_size: u64,
    last_temperature_scan: Option<Instant>,
    last_temperatures: Vec<TemperatureSnapshot>,
    sensor_retries: HashMap<PathBuf, SensorRetry>,
}

impl Backend {
    pub fn new() -> Self {
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        Self {
            page_size: if page_size > 0 {
                page_size as u64
            } else {
                4096
            },
            last_temperature_scan: None,
            last_temperatures: Vec::new(),
            sensor_retries: HashMap::new(),
        }
    }

    pub fn probe(&mut self, kind: ProbeKind) -> ProbeReport {
        let mut notes = Vec::new();

        match kind {
            ProbeKind::Cpu => {
                let value = read_cpu(Some(&mut notes));
                let available = value.is_some();
                let summary = value
                    .as_ref()
                    .map(|cpu| vec![format!("{} logical CPUs", cpu.logical_cpu_count)])
                    .unwrap_or_else(|| vec!["CPU counters unavailable".to_owned()]);
                let raw = value
                    .map(|cpu| {
                        vec![format!(
                            "total_jiffies={} idle_jiffies={} logical_cpu_count={}",
                            cpu.total_jiffies, cpu.idle_jiffies, cpu.logical_cpu_count
                        )]
                    })
                    .unwrap_or_default();
                ProbeReport {
                    available,
                    summary,
                    raw,
                    notes,
                }
            }
            ProbeKind::Memory => {
                let value = read_memory(Some(&mut notes));
                let available = value.is_some();
                let summary = value
                    .as_ref()
                    .map(|memory| {
                        vec![format!(
                            "used_bytes={} total_bytes={} swap_used_bytes={} swap_total_bytes={}",
                            memory.used_bytes,
                            memory.total_bytes,
                            memory.swap_used_bytes,
                            memory.swap_total_bytes
                        )]
                    })
                    .unwrap_or_else(|| vec!["memory counters unavailable".to_owned()]);
                let raw = value
                    .map(|memory| {
                        vec![format!(
                            "used_bytes={} total_bytes={} swap_used_bytes={} swap_total_bytes={}",
                            memory.used_bytes,
                            memory.total_bytes,
                            memory.swap_used_bytes,
                            memory.swap_total_bytes
                        )]
                    })
                    .unwrap_or_default();
                ProbeReport {
                    available,
                    summary,
                    raw,
                    notes,
                }
            }
            ProbeKind::Processes => {
                let values = read_processes(self.page_size, Some(&mut notes));
                let raw = values
                    .iter()
                    .map(|process| {
                        format!(
                            "pid={} name={:?} cpu_ticks={} rss_bytes={}",
                            process.pid, process.name, process.cpu_ticks, process.rss_bytes
                        )
                    })
                    .collect();
                ProbeReport {
                    available: collection_available(&values, &notes),
                    summary: vec![format!("{} processes", values.len())],
                    raw,
                    notes,
                }
            }
            ProbeKind::Network => {
                let values = read_networks(Some(&mut notes));
                let raw = values
                    .iter()
                    .map(|network| {
                        format!(
                            "name={} rx_bytes={} tx_bytes={}",
                            network.name, network.rx_bytes, network.tx_bytes
                        )
                    })
                    .collect();
                ProbeReport {
                    available: collection_available(&values, &notes),
                    summary: vec![format!(
                        "{} interfaces: {}",
                        values.len(),
                        names(values.iter().map(|item| item.name.as_str()))
                    )],
                    raw,
                    notes,
                }
            }
            ProbeKind::Disk => {
                let values = read_disks(Some(&mut notes));
                let raw = values
                    .iter()
                    .map(|disk| {
                        format!(
                            "name={} read_bytes={} write_bytes={}",
                            disk.name, disk.read_bytes, disk.write_bytes
                        )
                    })
                    .collect();
                ProbeReport {
                    available: collection_available(&values, &notes),
                    summary: vec![format!(
                        "{} devices: {}",
                        values.len(),
                        names(values.iter().map(|item| item.name.as_str()))
                    )],
                    raw,
                    notes,
                }
            }
            ProbeKind::Temperatures => {
                let values = self.collect_temperatures(Instant::now(), Some(&mut notes));
                let raw = values
                    .iter()
                    .map(|temperature| {
                        format!(
                            "name={} celsius={:.3}",
                            temperature.name, temperature.celsius
                        )
                    })
                    .collect();
                let summary = if values.is_empty() {
                    vec!["0 temperature sensors".to_owned()]
                } else {
                    values
                        .iter()
                        .map(|temperature| {
                            format!("{} {:.1}°C", temperature.name, temperature.celsius)
                        })
                        .collect()
                };
                ProbeReport {
                    available: collection_available(&values, &notes),
                    summary,
                    raw,
                    notes,
                }
            }
        }
    }

    fn collect_temperatures(
        &mut self,
        now: Instant,
        mut notes: Option<&mut Vec<String>>,
    ) -> Vec<TemperatureSnapshot> {
        if self
            .last_temperature_scan
            .is_some_and(|last| now.saturating_duration_since(last) < TEMPERATURE_INTERVAL)
        {
            probe_note(&mut notes, || {
                "temperature scan skipped: cached values are still fresh".to_owned()
            });
            return self.last_temperatures.clone();
        }
        self.last_temperature_scan = Some(now);

        let mut chips = Vec::new();
        let entries = match fs::read_dir("/sys/class/hwmon") {
            Ok(entries) => entries,
            Err(error) => {
                report_issue(&mut notes, || {
                    format!("cannot read /sys/class/hwmon: {error}")
                });
                return self.last_temperatures.clone();
            }
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let raw_name = read_trimmed(path.join("name"))
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| entry.file_name().to_string_lossy().into_owned());
            chips.push((
                entry.file_name().to_string_lossy().into_owned(),
                path,
                raw_name,
            ));
        }
        chips.sort_by(|a, b| a.0.cmp(&b.0));

        let mut raw_counts: HashMap<String, usize> = HashMap::new();
        for (_, _, raw) in &chips {
            *raw_counts.entry(raw.clone()).or_default() += 1;
        }
        let mut ordinals: HashMap<String, usize> = HashMap::new();
        let mut temperatures = Vec::new();
        let mut seen_sensors = Vec::new();

        for (_, chip_path, raw_name) in chips {
            let mut channels = Vec::new();
            let files = match fs::read_dir(&chip_path) {
                Ok(files) => files,
                Err(error) => {
                    report_issue(&mut notes, || {
                        format!("cannot read {}: {error}", chip_path.display())
                    });
                    continue;
                }
            };
            for file in files.flatten() {
                let file_name = file.file_name();
                let file_name = file_name.to_string_lossy();
                let Some(channel) = parse_temperature_channel(&file_name) else {
                    continue;
                };
                channels.push((channel, file.path()));
            }
            channels.sort_by_key(|(channel, _)| *channel);

            let mut representative = None;
            for (_, path) in channels {
                seen_sensors.push(path.clone());
                if self
                    .sensor_retries
                    .get(&path)
                    .is_some_and(|retry| now < retry.retry_at)
                {
                    probe_note(&mut notes, || {
                        format!("{} skipped: sensor is in retry backoff", path.display())
                    });
                    continue;
                }
                match read_trimmed(&path)
                    .and_then(|value| value.parse::<f64>().ok())
                    .filter(|value| value.is_finite())
                {
                    Some(milli_celsius) => {
                        self.sensor_retries.remove(&path);
                        representative = Some(milli_celsius / 1000.0);
                        break;
                    }
                    None => {
                        probe_note(&mut notes, || {
                            format!("{} skipped: unreadable or invalid value", path.display())
                        });
                        self.record_sensor_failure(path, now);
                    }
                }
            }

            if let Some(celsius) = representative {
                let ordinal = ordinals.entry(raw_name.clone()).or_default();
                *ordinal += 1;
                let friendly = friendly_chip_name(&raw_name);
                let display = if raw_counts.get(&raw_name).copied().unwrap_or(0) > 1 {
                    format!("{friendly} {ordinal}")
                } else {
                    friendly
                };
                temperatures.push(TemperatureSnapshot {
                    name: display,
                    celsius,
                });
            }
        }

        self.sensor_retries
            .retain(|path, _| seen_sensors.iter().any(|seen| seen == path));
        temperatures.sort_by(|a, b| a.name.cmp(&b.name));
        self.last_temperatures = temperatures.clone();
        temperatures
    }

    fn record_sensor_failure(&mut self, path: PathBuf, now: Instant) {
        let failures = self
            .sensor_retries
            .get(&path)
            .map_or(1, |retry| (retry.failures + 1).min(7));
        let seconds =
            (5_u64.saturating_mul(1_u64 << (failures - 1))).min(MAX_SENSOR_BACKOFF.as_secs());
        devlog::log(format_args!(
            "temperature sensor failure path={} retry_in={}s failures={failures}",
            path.display(),
            seconds
        ));
        self.sensor_retries.insert(
            path,
            SensorRetry {
                failures,
                retry_at: now + Duration::from_secs(seconds),
            },
        );
    }
}

impl Default for Backend {
    fn default() -> Self {
        Self::new()
    }
}

impl Collector for Backend {
    fn collect(&mut self) -> RawSnapshot {
        let collected_at = Instant::now();
        let cpu = devlog::timed("collector.cpu", || read_cpu(None));
        let memory = devlog::timed("collector.memory", || read_memory(None));
        let processes = devlog::timed("collector.processes", || {
            read_processes(self.page_size, None)
        });
        let networks = devlog::timed("collector.network", || read_networks(None));
        let disks = devlog::timed("collector.disk", || read_disks(None));
        let temperatures = devlog::timed("collector.temperatures", || {
            self.collect_temperatures(Instant::now(), None)
        });

        RawSnapshot {
            collected_at,
            cpu,
            memory,
            processes,
            networks,
            disks,
            temperatures,
        }
    }
}

fn read_cpu(mut notes: Option<&mut Vec<String>>) -> Option<CpuCounter> {
    let text = match fs::read_to_string("/proc/stat") {
        Ok(text) => text,
        Err(error) => {
            report_issue(&mut notes, || format!("cannot read /proc/stat: {error}"));
            return None;
        }
    };
    let mut lines = text.lines();
    let Some(first) = lines.next() else {
        report_issue(&mut notes, || "/proc/stat is empty".to_owned());
        return None;
    };
    let mut fields = first.split_whitespace();
    if fields.next() != Some("cpu") {
        report_issue(&mut notes, || {
            "/proc/stat has no aggregate cpu row".to_owned()
        });
        return None;
    }
    let values: Vec<u64> = fields.filter_map(|value| value.parse().ok()).collect();
    if values.len() < 5 {
        report_issue(&mut notes, || {
            format!(
                "/proc/stat aggregate cpu row has only {} counters",
                values.len()
            )
        });
        return None;
    }
    let total_jiffies = values.iter().copied().sum();
    let idle_jiffies = values.get(3).copied().unwrap_or(0) + values.get(4).copied().unwrap_or(0);
    let logical_cpus = lines
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| {
            name.strip_prefix("cpu").is_some_and(|suffix| {
                !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
            })
        })
        .count()
        .max(1);
    Some(CpuCounter {
        total_jiffies,
        idle_jiffies,
        logical_cpu_count: logical_cpus,
    })
}

fn read_memory(mut notes: Option<&mut Vec<String>>) -> Option<MemorySnapshot> {
    let text = match fs::read_to_string("/proc/meminfo") {
        Ok(text) => text,
        Err(error) => {
            report_issue(&mut notes, || format!("cannot read /proc/meminfo: {error}"));
            return None;
        }
    };
    let mut values = HashMap::new();
    for line in text.lines() {
        let Some((key, rest)) = line.split_once(':') else {
            continue;
        };
        let Some(value) = rest
            .split_whitespace()
            .next()
            .and_then(|value| value.parse::<u64>().ok())
        else {
            continue;
        };
        values.insert(key, value.saturating_mul(1024));
    }
    let Some(total_bytes) = values.get("MemTotal").copied() else {
        report_issue(&mut notes, || "/proc/meminfo has no MemTotal".to_owned());
        return None;
    };
    let available_bytes = values.get("MemAvailable").copied().unwrap_or(0);
    let swap_total_bytes = values.get("SwapTotal").copied().unwrap_or(0);
    let swap_free_bytes = values.get("SwapFree").copied().unwrap_or(0);
    Some(MemorySnapshot {
        used_bytes: total_bytes.saturating_sub(available_bytes),
        total_bytes,
        swap_used_bytes: swap_total_bytes.saturating_sub(swap_free_bytes),
        swap_total_bytes,
    })
}

fn read_processes(page_size: u64, mut notes: Option<&mut Vec<String>>) -> Vec<ProcessCounter> {
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
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        let Ok(stat) = fs::read_to_string(entry.path().join("stat")) else {
            unreadable += 1;
            continue;
        };
        if let Some(process) = parse_process_stat(pid, &stat, page_size) {
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

fn parse_process_stat(pid: u32, text: &str, page_size: u64) -> Option<ProcessCounter> {
    let left = text.find('(')?;
    let right = text.rfind(')')?;
    if right <= left {
        return None;
    }
    let fields: Vec<&str> = text[right + 1..].split_whitespace().collect();
    let user = fields.get(11)?.parse::<u64>().ok()?;
    let system = fields.get(12)?.parse::<u64>().ok()?;
    let rss_pages = fields.get(21)?.parse::<u64>().ok()?;
    Some(ProcessCounter {
        pid,
        name: text[left + 1..right].to_owned(),
        cpu_ticks: user.saturating_add(system),
        rss_bytes: rss_pages.saturating_mul(page_size),
    })
}

fn read_networks(mut notes: Option<&mut Vec<String>>) -> Vec<NetworkCounter> {
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
            name,
            rx_bytes: received_bytes,
            tx_bytes: transmitted_bytes,
        });
    }
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    rows
}

fn read_disks(mut notes: Option<&mut Vec<String>>) -> Vec<DiskCounter> {
    let mut rows = Vec::new();
    let entries = match fs::read_dir("/sys/block") {
        Ok(entries) => entries,
        Err(error) => {
            report_issue(&mut notes, || format!("cannot read /sys/block: {error}"));
            return rows;
        }
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !entry.path().join("device").exists() {
            probe_note(&mut notes, || {
                format!("{name} skipped: no physical device link")
            });
            continue;
        }
        let Some(stat) = read_trimmed(entry.path().join("stat")) else {
            probe_note(&mut notes, || format!("{name} skipped: stat is unreadable"));
            continue;
        };
        let fields: Vec<&str> = stat.split_whitespace().collect();
        if fields.len() < 7 {
            probe_note(&mut notes, || {
                format!("{name} skipped: stat has only {} fields", fields.len())
            });
            continue;
        }
        let read_sectors = fields
            .get(2)
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);
        let written_sectors = fields
            .get(6)
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);
        rows.push(DiskCounter {
            name,
            read_bytes: read_sectors.saturating_mul(512),
            write_bytes: written_sectors.saturating_mul(512),
        });
    }
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    rows
}

fn parse_temperature_channel(name: &str) -> Option<u32> {
    let channel = name.strip_prefix("temp")?.strip_suffix("_input")?;
    channel.parse().ok()
}

fn friendly_chip_name(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    match lower.as_str() {
        "acpitz" => "ACPI".to_owned(),
        "coretemp" | "k10temp" | "zenpower" => "CPU".to_owned(),
        "nvme" => "NVMe".to_owned(),
        "amdgpu" | "nouveau" => "GPU".to_owned(),
        _ if lower.starts_with("iwlwifi") => "WiFi".to_owned(),
        _ => title_case(raw),
    }
}

fn title_case(raw: &str) -> String {
    raw.split(['_', '-'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn read_u64(path: impl AsRef<Path>) -> Option<u64> {
    read_trimmed(path)?.parse().ok()
}

fn read_trimmed(path: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|text| text.trim().to_owned())
}

fn collection_available<T>(values: &[T], notes: &[String]) -> bool {
    !values.is_empty() || !notes.iter().any(|note| note.starts_with("cannot read "))
}

fn names<'a>(values: impl Iterator<Item = &'a str>) -> String {
    let names = values.collect::<Vec<_>>();
    if names.is_empty() {
        "none".to_owned()
    } else {
        names.join(", ")
    }
}

fn probe_note(notes: &mut Option<&mut Vec<String>>, message: impl FnOnce() -> String) {
    if let Some(notes) = notes.as_deref_mut() {
        notes.push(message());
    }
}

fn report_issue(notes: &mut Option<&mut Vec<String>>, message: impl FnOnce() -> String) {
    if notes.is_none() && !devlog::enabled() {
        return;
    }

    let message = message();
    devlog::log(format_args!("collector issue: {message}"));
    if let Some(notes) = notes.as_deref_mut() {
        notes.push(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_process_name_with_spaces() {
        let text =
            "42 (name with space) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22";
        let process = parse_process_stat(42, text, 4096).unwrap();
        assert_eq!(process.name, "name with space");
        assert_eq!(process.cpu_ticks, 23);
    }

    #[test]
    fn temperature_channel_is_capability_based() {
        assert_eq!(parse_temperature_channel("temp12_input"), Some(12));
        assert_eq!(parse_temperature_channel("temp12_label"), None);
    }
}
