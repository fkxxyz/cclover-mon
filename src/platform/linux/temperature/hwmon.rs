use std::fs::{self, File};
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::core::devlog;
use crate::core::model::TemperatureSnapshot;

use super::super::diagnostics::{probe_note, report_issue};

const MAX_SENSOR_BACKOFF: Duration = Duration::from_secs(300);
const DISCOVERY_RETRY: Duration = Duration::from_secs(5);

pub(super) struct Collector {
    chips: Vec<Chip>,
    discovered: bool,
    retry_discovery_at: Option<Instant>,
}

struct Chip {
    name: String,
    channels: Vec<Channel>,
}

struct Channel {
    id: String,
    path: PathBuf,
    file: File,
    failures: u32,
    retry_at: Option<Instant>,
}

impl Collector {
    pub(super) fn new() -> Self {
        Self {
            chips: Vec::new(),
            discovered: false,
            retry_discovery_at: None,
        }
    }

    pub(super) fn collect(
        &mut self,
        now: Instant,
        mut notes: Option<&mut Vec<String>>,
    ) -> Vec<TemperatureSnapshot> {
        if !self.discovered
            && self
                .retry_discovery_at
                .is_none_or(|retry_at| now >= retry_at)
        {
            match discover(notes.as_deref_mut()) {
                Some(chips) => {
                    self.chips = chips;
                    self.discovered = true;
                    self.retry_discovery_at = None;
                }
                None => {
                    self.retry_discovery_at = Some(now + DISCOVERY_RETRY);
                }
            }
        }

        let mut temperatures = Vec::new();
        for chip in &mut self.chips {
            for channel in &mut chip.channels {
                if channel.retry_at.is_some_and(|retry_at| now < retry_at) {
                    probe_note(&mut notes, || {
                        format!(
                            "{} skipped: sensor is in retry backoff",
                            channel.path.display()
                        )
                    });
                    continue;
                }

                match read_milli_celsius(&channel.file) {
                    Some(milli_celsius) => {
                        channel.failures = 0;
                        channel.retry_at = None;
                        temperatures.push(TemperatureSnapshot {
                            id: channel.id.clone(),
                            name: chip.name.clone(),
                            celsius: milli_celsius / 1000.0,
                        });
                        break;
                    }
                    None => {
                        probe_note(&mut notes, || {
                            format!(
                                "{} skipped: unreadable or invalid value",
                                channel.path.display()
                            )
                        });
                        record_sensor_failure(channel, now);
                    }
                }
            }
        }

        temperatures
    }
}

fn discover(mut notes: Option<&mut Vec<String>>) -> Option<Vec<Chip>> {
    let entries = match fs::read_dir("/sys/class/hwmon") {
        Ok(entries) => entries,
        Err(error) => {
            report_issue(&mut notes, || {
                format!("cannot read /sys/class/hwmon: {error}")
            });
            return None;
        }
    };

    let mut paths = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    paths.sort();

    let mut chips = Vec::new();
    for chip_path in paths {
        let raw_name = read_trimmed(chip_path.join("name"))
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| {
                chip_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            });

        let files = match fs::read_dir(&chip_path) {
            Ok(files) => files,
            Err(error) => {
                report_issue(&mut notes, || {
                    format!("cannot read {}: {error}", chip_path.display())
                });
                continue;
            }
        };

        let mut candidates = Vec::new();
        for file in files.flatten() {
            let file_name = file.file_name();
            let file_name = file_name.to_string_lossy();
            let Some(channel) = parse_temperature_channel(&file_name) else {
                continue;
            };
            candidates.push((channel, file.path()));
        }
        candidates.sort_by_key(|(channel, _)| *channel);

        let device_identity = stable_device_identity(&chip_path);
        let mut channels = Vec::new();
        for (channel, path) in candidates {
            match File::open(&path) {
                Ok(file) => channels.push(Channel {
                    id: format!("hwmon:{device_identity}:{raw_name}:temp{channel}"),
                    path,
                    file,
                    failures: 0,
                    retry_at: None,
                }),
                Err(error) => report_issue(&mut notes, || {
                    format!("cannot open {}: {error}", path.display())
                }),
            }
        }

        if !channels.is_empty() {
            chips.push(Chip {
                name: friendly_chip_name(&raw_name),
                channels,
            });
        }
    }

    Some(chips)
}

fn record_sensor_failure(channel: &mut Channel, now: Instant) {
    channel.failures = (channel.failures + 1).min(7);
    let seconds =
        (5_u64.saturating_mul(1_u64 << (channel.failures - 1))).min(MAX_SENSOR_BACKOFF.as_secs());
    channel.retry_at = Some(now + Duration::from_secs(seconds));
    devlog::log(format_args!(
        "temperature sensor failure path={} retry_in={}s failures={}",
        channel.path.display(),
        seconds,
        channel.failures
    ));
}

fn parse_temperature_channel(name: &str) -> Option<u32> {
    let channel = name.strip_prefix("temp")?.strip_suffix("_input")?;
    channel.parse().ok()
}

fn read_milli_celsius(file: &File) -> Option<f64> {
    let mut buffer = [0_u8; 64];
    let bytes = file.read_at(&mut buffer, 0).ok()?;
    let text = std::str::from_utf8(&buffer[..bytes]).ok()?;
    parse_milli_celsius(text)
}

fn parse_milli_celsius(text: &str) -> Option<f64> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

fn stable_device_identity(chip_path: &Path) -> String {
    if let Ok(device_path) = fs::canonicalize(chip_path.join("device")) {
        return device_path.to_string_lossy().into_owned();
    }

    let canonical = fs::canonicalize(chip_path).unwrap_or_else(|_| chip_path.to_path_buf());
    strip_hwmon_suffix(&canonical)
        .to_string_lossy()
        .into_owned()
}

fn strip_hwmon_suffix(path: &Path) -> &Path {
    let Some(parent) = path.parent() else {
        return path;
    };
    if parent.file_name().is_some_and(|name| name == "hwmon") {
        return parent.parent().unwrap_or(path);
    }
    path
}

fn friendly_chip_name(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    match lower.as_str() {
        "acpitz" => "ACPI".to_owned(),
        "coretemp" | "k10temp" | "zenpower" => "CPU".to_owned(),
        "nvme" => "NVMe".to_owned(),
        "amdgpu" | "nouveau" | "i915" | "xe" => "GPU".to_owned(),
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

    #[test]
    fn strips_volatile_hwmon_directory_from_fallback_identity() {
        let path = Path::new("/sys/devices/pci0000:00/0000:00:01.0/hwmon/hwmon7");
        assert_eq!(
            strip_hwmon_suffix(path),
            Path::new("/sys/devices/pci0000:00/0000:00:01.0")
        );
    }

    #[test]
    fn intel_gpu_names_share_the_gpu_presentation_semantic() {
        assert_eq!(friendly_chip_name("i915"), "GPU");
        assert_eq!(friendly_chip_name("xe"), "GPU");
    }
}
