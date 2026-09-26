use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::core::devlog;
use crate::core::model::TemperatureSnapshot;

use super::diagnostics::{probe_note, report_issue};

const TEMPERATURE_INTERVAL: Duration = Duration::from_secs(2);
const MAX_SENSOR_BACKOFF: Duration = Duration::from_secs(300);

#[derive(Debug, Clone)]
struct SensorRetry {
    failures: u32,
    retry_at: Instant,
}

pub(super) struct Collector {
    last_scan: Option<Instant>,
    last_values: Vec<TemperatureSnapshot>,
    sensor_retries: HashMap<PathBuf, SensorRetry>,
}

impl Collector {
    pub(super) fn new() -> Self {
        Self {
            last_scan: None,
            last_values: Vec::new(),
            sensor_retries: HashMap::new(),
        }
    }

    pub(super) fn collect(
        &mut self,
        now: Instant,
        mut notes: Option<&mut Vec<String>>,
    ) -> Vec<TemperatureSnapshot> {
        if self
            .last_scan
            .is_some_and(|last| now.saturating_duration_since(last) < TEMPERATURE_INTERVAL)
        {
            probe_note(&mut notes, || {
                "temperature scan skipped: cached values are still fresh".to_owned()
            });
            return self.last_values.clone();
        }
        self.last_scan = Some(now);

        let mut chips = Vec::new();
        let entries = match fs::read_dir("/sys/class/hwmon") {
            Ok(entries) => entries,
            Err(error) => {
                report_issue(&mut notes, || {
                    format!("cannot read /sys/class/hwmon: {error}")
                });
                return self.last_values.clone();
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
                match read_trimmed(&path).as_deref().and_then(parse_milli_celsius) {
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
        self.last_values = temperatures.clone();
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

fn parse_temperature_channel(name: &str) -> Option<u32> {
    let channel = name.strip_prefix("temp")?.strip_suffix("_input")?;
    channel.parse().ok()
}

fn parse_milli_celsius(text: &str) -> Option<f64> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
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

fn read_trimmed(path: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|text| text.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temperature_channel_is_capability_based() {
        assert_eq!(parse_temperature_channel("temp12_input"), Some(12));
        assert_eq!(parse_temperature_channel("temp12_label"), None);
    }

    #[test]
    fn parses_temperature_value_without_live_sysfs() {
        assert_eq!(parse_milli_celsius("42000\n"), Some(42000.0));
        assert_eq!(parse_milli_celsius("NaN\n"), None);
    }
}
