use std::fs::{self, File};
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::core::devlog;
use crate::core::model::{Collection, CollectionUnavailable, FanId, FanSnapshot};

use super::diagnostics::{probe_note, report_issue, unavailable_from_io};

const MAX_SENSOR_BACKOFF: Duration = Duration::from_secs(300);
const DISCOVERY_RETRY: Duration = Duration::from_secs(5);

pub(super) struct Collector {
    channels: Vec<Channel>,
    discovered: bool,
    discovery_degraded: bool,
    discovery_unavailable: CollectionUnavailable,
    retry_discovery_at: Option<Instant>,
}

struct Channel {
    id: FanId,
    name: String,
    path: PathBuf,
    file: File,
    failures: u32,
    retry_at: Option<Instant>,
}

impl Collector {
    pub(super) fn new() -> Self {
        Self {
            channels: Vec::new(),
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
    ) -> Collection<Vec<FanSnapshot>> {
        self.collect_from(now, Path::new("/sys/class/hwmon"), notes)
    }

    fn collect_from(
        &mut self,
        now: Instant,
        hwmon_root: &Path,
        mut notes: Option<&mut Vec<String>>,
    ) -> Collection<Vec<FanSnapshot>> {
        if !self.discovered
            && self
                .retry_discovery_at
                .is_none_or(|retry_at| now >= retry_at)
        {
            match discover(hwmon_root, notes.as_deref_mut()) {
                Ok((channels, degraded)) => {
                    self.channels = channels;
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

        let mut fans = Vec::with_capacity(self.channels.len());
        let mut degraded = self.discovery_degraded;
        for channel in &mut self.channels {
            if channel.retry_at.is_some_and(|retry_at| now < retry_at) {
                degraded = true;
                probe_note(&mut notes, || {
                    format!(
                        "{} skipped: fan sensor is in retry backoff",
                        channel.path.display()
                    )
                });
                continue;
            }

            match read_rpm(&channel.file) {
                Some(rpm) => {
                    channel.failures = 0;
                    channel.retry_at = None;
                    fans.push(FanSnapshot {
                        id: channel.id.clone(),
                        name: channel.name.clone(),
                        rpm,
                    });
                }
                None => {
                    degraded = true;
                    probe_note(&mut notes, || {
                        format!(
                            "{} skipped: unreadable or invalid RPM",
                            channel.path.display()
                        )
                    });
                    record_sensor_failure(channel, now);
                }
            }
        }

        if degraded {
            Collection::degraded(fans)
        } else {
            Collection::available(fans)
        }
    }
}

fn discover(
    hwmon_root: &Path,
    mut notes: Option<&mut Vec<String>>,
) -> Result<(Vec<Channel>, bool), CollectionUnavailable> {
    let entries = fs::read_dir(hwmon_root).map_err(|error| {
        report_issue(&mut notes, || {
            format!("cannot read {}: {error}", hwmon_root.display())
        });
        unavailable_from_io(&error)
    })?;

    let mut chip_paths = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    chip_paths.sort();

    let mut channels = Vec::new();
    let mut degraded = false;
    for chip_path in chip_paths {
        let raw_name = read_trimmed(chip_path.join("name"))
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| {
                chip_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            });
        let device_identity = stable_device_identity(&chip_path);
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

        let mut candidates = files
            .flatten()
            .filter_map(|file| {
                let file_name = file.file_name();
                let file_name = file_name.to_string_lossy();
                parse_fan_channel(&file_name).map(|channel| (channel, file.path()))
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|(channel, _)| *channel);

        for (channel, path) in candidates {
            let label = read_trimmed(chip_path.join(format!("fan{channel}_label")))
                .filter(|label| !label.is_empty())
                .unwrap_or_else(|| format!("{} Fan #{}", friendly_chip_name(&raw_name), channel));
            match File::open(&path) {
                Ok(file) => channels.push(Channel {
                    id: FanId::from_opaque_key(format!(
                        "hwmon:{device_identity}:{raw_name}:fan{channel}"
                    )),
                    name: label,
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
    }

    Ok((channels, degraded))
}

fn parse_fan_channel(name: &str) -> Option<u32> {
    name.strip_prefix("fan")?
        .strip_suffix("_input")?
        .parse()
        .ok()
}

fn read_rpm(file: &File) -> Option<u64> {
    let mut buffer = [0_u8; 64];
    let bytes = file.read_at(&mut buffer, 0).ok()?;
    let text = std::str::from_utf8(&buffer[..bytes]).ok()?;
    text.trim().parse::<u64>().ok()
}

fn record_sensor_failure(channel: &mut Channel, now: Instant) {
    channel.failures = (channel.failures + 1).min(7);
    let seconds =
        (5_u64.saturating_mul(1_u64 << (channel.failures - 1))).min(MAX_SENSOR_BACKOFF.as_secs());
    channel.retry_at = Some(now + Duration::from_secs(seconds));
    devlog::log(format_args!(
        "fan sensor failure path={} retry_in={}s failures={}",
        channel.path.display(),
        seconds,
        channel.failures
    ));
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
    match raw.to_ascii_lowercase().as_str() {
        "coretemp" | "k10temp" | "zenpower" => "CPU".to_owned(),
        "amdgpu" | "nouveau" | "i915" | "xe" => "GPU".to_owned(),
        other => other
            .split(['_', '-'])
            .filter(|part| !part.is_empty())
            .map(|part| {
                let mut chars = part.chars();
                chars
                    .next()
                    .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

fn read_trimmed(path: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|text| text.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::linux::test_support::Fixture;

    #[test]
    fn fan_channel_is_capability_based() {
        assert_eq!(parse_fan_channel("fan12_input"), Some(12));
        assert_eq!(parse_fan_channel("fan12_label"), None);
    }

    #[test]
    fn discovers_labeled_and_unlabeled_fans_with_stable_identity() {
        let fixture = Fixture::new("fan-hwmon-discovery");
        fixture.dir("devices/board");
        fixture.dir("hwmon/hwmon7");
        fixture.symlink_to("devices/board", "hwmon/hwmon7/device");
        fixture.write("hwmon/hwmon7/name", "nct6798\n");
        fixture.write("hwmon/hwmon7/fan1_input", "1320\n");
        fixture.write("hwmon/hwmon7/fan1_label", "CPU Fan\n");
        fixture.write("hwmon/hwmon7/fan2_input", "0\n");

        let mut collector = Collector::new();
        let outcome = collector.collect_from(Instant::now(), &fixture.path().join("hwmon"), None);
        let Collection::Available(values) = outcome else {
            panic!("expected available fan collection");
        };

        assert_eq!(values.len(), 2);
        assert_eq!(values[0].name, "CPU Fan");
        assert_eq!(values[0].rpm, 1320);
        assert!(values[0].id.as_opaque_key().contains("nct6798:fan1"));
        assert_eq!(values[1].name, "Nct6798 Fan #2");
        assert_eq!(values[1].rpm, 0);
    }

    #[test]
    fn missing_hwmon_root_is_unavailable() {
        let fixture = Fixture::new("fan-hwmon-missing");
        let mut collector = Collector::new();
        assert!(matches!(
            collector.collect_from(Instant::now(), &fixture.path().join("missing"), None),
            Collection::Unavailable(_)
        ));
    }
}
