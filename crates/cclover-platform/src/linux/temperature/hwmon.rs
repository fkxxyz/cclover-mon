use std::fs::{self, File};
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use cclover_core::devlog;
use cclover_core::model::{Collection, CollectionUnavailable, TemperatureId, TemperatureSnapshot};

use super::super::diagnostics::{probe_note, report_issue, unavailable_from_io};
use super::Observation;
use crate::linux::PhysicalDeviceId;

const MAX_SENSOR_BACKOFF: Duration = Duration::from_secs(300);
const DISCOVERY_RETRY: Duration = Duration::from_secs(5);

pub(super) struct Collector {
    chips: Vec<Chip>,
    discovered: bool,
    discovery_degraded: bool,
    discovery_unavailable: CollectionUnavailable,
    retry_discovery_at: Option<Instant>,
}

struct Chip {
    name: String,
    physical_device: Option<PhysicalDeviceId>,
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
            discovery_degraded: false,
            discovery_unavailable: CollectionUnavailable::Unavailable,
            retry_discovery_at: None,
        }
    }

    pub(super) fn collect(
        &mut self,
        now: Instant,
        notes: Option<&mut Vec<String>>,
    ) -> Collection<Vec<Observation>> {
        self.collect_from(now, Path::new("/sys/class/hwmon"), notes)
    }

    fn collect_from(
        &mut self,
        now: Instant,
        hwmon_root: &Path,
        mut notes: Option<&mut Vec<String>>,
    ) -> Collection<Vec<Observation>> {
        if !self.discovered
            && self
                .retry_discovery_at
                .is_none_or(|retry_at| now >= retry_at)
        {
            match discover(hwmon_root, notes.as_deref_mut()) {
                Ok((chips, degraded)) => {
                    self.chips = chips;
                    self.discovered = true;
                    self.discovery_degraded = degraded;
                    self.retry_discovery_at = None;
                }
                Err(reason) => {
                    self.discovery_unavailable = reason;
                    self.retry_discovery_at = Some(now + DISCOVERY_RETRY);
                }
            }
        }

        if !self.discovered {
            return Collection::unavailable(self.discovery_unavailable);
        }

        let mut temperatures = Vec::new();
        let mut degraded = self.discovery_degraded;
        for chip in &mut self.chips {
            for channel in &mut chip.channels {
                if channel.retry_at.is_some_and(|retry_at| now < retry_at) {
                    degraded = true;
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
                        temperatures.push(Observation {
                            physical_device: chip.physical_device.clone(),
                            snapshot: TemperatureSnapshot {
                                id: TemperatureId::from_opaque_key(channel.id.clone()),
                                name: chip.name.clone(),
                                celsius: milli_celsius / 1000.0,
                            },
                        });
                        break;
                    }
                    None => {
                        degraded = true;
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

        if degraded {
            Collection::degraded(temperatures)
        } else {
            Collection::available(temperatures)
        }
    }
}

fn discover(
    hwmon_root: &Path,
    mut notes: Option<&mut Vec<String>>,
) -> Result<(Vec<Chip>, bool), CollectionUnavailable> {
    let entries = match fs::read_dir(hwmon_root) {
        Ok(entries) => entries,
        Err(error) => {
            report_issue(&mut notes, || {
                format!("cannot read {}: {error}", hwmon_root.display())
            });
            return Err(unavailable_from_io(&error));
        }
    };

    let mut paths = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    paths.sort();

    let mut chips = Vec::new();
    let mut degraded = false;
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
                degraded = true;
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

        let physical_device = PhysicalDeviceId::from_path(&chip_path.join("device"));
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
                Err(error) => {
                    degraded = true;
                    report_issue(&mut notes, || {
                        format!("cannot open {}: {error}", path.display())
                    });
                }
            }
        }

        if !channels.is_empty() {
            chips.push(Chip {
                name: friendly_chip_name(&raw_name),
                physical_device,
                channels,
            });
        }
    }

    Ok((chips, degraded))
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
    use crate::linux::test_support::Fixture;

    fn add_hwmon_chip(
        fixture: &Fixture,
        directory: &str,
        device: &str,
        name: &str,
        temperature: &str,
    ) {
        fixture.dir(format!("devices/{device}"));
        fixture.dir(format!("hwmon/{directory}"));
        fixture.symlink_to(
            format!("devices/{device}"),
            format!("hwmon/{directory}/device"),
        );
        fixture.write(format!("hwmon/{directory}/name"), format!("{name}\n"));
        fixture.write(
            format!("hwmon/{directory}/temp1_input"),
            format!("{temperature}\n"),
        );
    }

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

    #[test]
    fn discovers_hwmon_sensors_without_embedding_gpu_ownership_policy() {
        let fixture = Fixture::new("hwmon-discovery");
        fixture.dir("hwmon");
        add_hwmon_chip(&fixture, "hwmon0", "cpu", "coretemp", "42000");
        add_hwmon_chip(&fixture, "hwmon1", "gpu", "amdgpu", "63000");

        let mut collector = Collector::new();
        let outcome = collector.collect_from(Instant::now(), &fixture.path().join("hwmon"), None);
        let Collection::Available(values) = outcome else {
            panic!("expected available hwmon collection");
        };

        assert_eq!(values.len(), 2);
        assert_eq!(values[0].snapshot.name, "CPU");
        assert_eq!(values[0].snapshot.celsius, 42.0);
        assert!(
            values[0]
                .snapshot
                .id
                .as_opaque_key()
                .contains("coretemp:temp1")
        );
        assert_eq!(values[1].snapshot.name, "GPU");
        assert_eq!(values[1].snapshot.celsius, 63.0);
        assert!(values[1].physical_device.is_some());
    }

    #[test]
    fn invalid_sensor_degrades_but_preserves_valid_hwmon_values() {
        let fixture = Fixture::new("hwmon-partial-failure");
        fixture.dir("hwmon");
        add_hwmon_chip(&fixture, "hwmon0", "cpu", "coretemp", "42000");
        add_hwmon_chip(&fixture, "hwmon1", "nvme", "nvme", "not-a-number");

        let mut collector = Collector::new();
        let outcome = collector.collect_from(Instant::now(), &fixture.path().join("hwmon"), None);
        let Collection::Degraded(values) = outcome else {
            panic!("expected degraded hwmon collection");
        };

        assert_eq!(values.len(), 1);
        assert_eq!(values[0].snapshot.name, "CPU");
    }

    #[test]
    fn missing_hwmon_root_is_unavailable() {
        let fixture = Fixture::new("hwmon-missing-root");
        let mut collector = Collector::new();
        assert!(matches!(
            collector.collect_from(Instant::now(), &fixture.path().join("missing"), None),
            Collection::Unavailable(_)
        ));
    }
}
