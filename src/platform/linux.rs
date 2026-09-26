use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::core::model::{
    CpuCounter, DiskCounter, MemorySnapshot, NetworkCounter, ProcessCounter, RawSnapshot,
    TemperatureSnapshot,
};
use crate::platform::Collector;

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

    fn collect_temperatures(&mut self, now: Instant) -> Vec<TemperatureSnapshot> {
        if self
            .last_temperature_scan
            .is_some_and(|last| now.saturating_duration_since(last) < TEMPERATURE_INTERVAL)
        {
            return self.last_temperatures.clone();
        }
        self.last_temperature_scan = Some(now);

        let mut chips = Vec::new();
        let Ok(entries) = fs::read_dir("/sys/class/hwmon") else {
            return self.last_temperatures.clone();
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
            let Ok(files) = fs::read_dir(&chip_path) else {
                continue;
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
                    None => self.record_sensor_failure(path, now),
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
        RawSnapshot {
            collected_at: Instant::now(),
            cpu: read_cpu(),
            memory: read_memory(),
            processes: read_processes(self.page_size),
            networks: read_networks(),
            disks: read_disks(),
            temperatures: self.collect_temperatures(Instant::now()),
        }
    }
}

fn read_cpu() -> Option<CpuCounter> {
    let text = fs::read_to_string("/proc/stat").ok()?;
    let mut lines = text.lines();
    let first = lines.next()?;
    let mut fields = first.split_whitespace();
    if fields.next()? != "cpu" {
        return None;
    }
    let values: Vec<u64> = fields.filter_map(|value| value.parse().ok()).collect();
    if values.len() < 5 {
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

fn read_memory() -> Option<MemorySnapshot> {
    let text = fs::read_to_string("/proc/meminfo").ok()?;
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
    let total_bytes = *values.get("MemTotal")?;
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

fn read_processes(page_size: u64) -> Vec<ProcessCounter> {
    let mut processes = Vec::new();
    let Ok(entries) = fs::read_dir("/proc") else {
        return processes;
    };
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        let Ok(stat) = fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        if let Some(process) = parse_process_stat(pid, &stat, page_size) {
            processes.push(process);
        }
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

fn read_networks() -> Vec<NetworkCounter> {
    let mut rows = Vec::new();
    let Ok(entries) = fs::read_dir("/sys/class/net") else {
        return rows;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.join("device").exists() {
            continue;
        }
        if read_trimmed(path.join("operstate")).as_deref() != Some("up") {
            continue;
        }
        let Some(received_bytes) = read_u64(path.join("statistics/rx_bytes")) else {
            continue;
        };
        let Some(transmitted_bytes) = read_u64(path.join("statistics/tx_bytes")) else {
            continue;
        };
        rows.push(NetworkCounter {
            name: entry.file_name().to_string_lossy().into_owned(),
            rx_bytes: received_bytes,
            tx_bytes: transmitted_bytes,
        });
    }
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    rows
}

fn read_disks() -> Vec<DiskCounter> {
    let mut rows = Vec::new();
    let Ok(entries) = fs::read_dir("/sys/block") else {
        return rows;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !entry.path().join("device").exists() {
            continue;
        }
        let Some(stat) = read_trimmed(entry.path().join("stat")) else {
            continue;
        };
        let fields: Vec<&str> = stat.split_whitespace().collect();
        if fields.len() < 7 {
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
