use std::fs;
use std::path::{Path, PathBuf};

use crate::core::model::{Collection, GpuId, GpuSnapshot};

use super::diagnostics::{probe_note, unavailable_from_io};

pub(super) fn collect(
    nvidia: Collection<Vec<GpuSnapshot>>,
    notes: Option<&mut Vec<String>>,
) -> Collection<Vec<GpuSnapshot>> {
    merge_sources(collect_amd(Path::new("/sys/class/drm"), notes), nvidia)
}

fn collect_amd(
    drm_root: &Path,
    mut notes: Option<&mut Vec<String>>,
) -> Collection<Vec<GpuSnapshot>> {
    let entries = match fs::read_dir(drm_root) {
        Ok(entries) => entries,
        Err(error) => {
            probe_note(&mut notes, || format!("DRM discovery failed: {error}"));
            return Collection::unavailable(unavailable_from_io(&error));
        }
    };

    let mut values = Vec::new();
    let mut degraded = false;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("card") || name[4..].contains('-') {
            continue;
        }

        let device = entry.path().join("device");
        if !is_amdgpu(&device) {
            continue;
        }
        match read_amd_device(&device) {
            Ok(snapshot) => values.push(snapshot),
            Err(error) => {
                degraded = true;
                probe_note(&mut notes, || {
                    format!("AMD GPU {name} unavailable: {error}")
                });
            }
        }
    }

    if degraded {
        Collection::degraded(values)
    } else {
        Collection::available(values)
    }
}

fn is_amdgpu(device: &Path) -> bool {
    fs::read_link(device.join("driver"))
        .ok()
        .and_then(|path| path.file_name().map(|name| name == "amdgpu"))
        .unwrap_or(false)
}

fn read_amd_device(device: &Path) -> Result<GpuSnapshot, String> {
    let canonical = fs::canonicalize(device).map_err(|error| error.to_string())?;
    let id = GpuId::from_opaque_key(format!("drm:{}", canonical.display()));
    let name = read_device_name(device).unwrap_or_else(|| "AMD GPU".to_owned());
    let memory_used_bytes = read_optional_u64(device.join("mem_info_vram_used"));
    let memory_total_bytes = read_optional_u64(device.join("mem_info_vram_total"));
    let utilization_percent = read_optional_u64(device.join("gpu_busy_percent"))
        .map(|value| (value as f64).clamp(0.0, 100.0));
    let core_clock_mhz = read_active_dpm_clock(device.join("pp_dpm_sclk"));
    let hwmon = first_hwmon(device);
    let temperature_celsius = hwmon
        .as_ref()
        .and_then(|path| read_first_numbered(path, "temp", "_input"))
        .map(|value| value as f64 / 1000.0);
    let power_watts = hwmon
        .as_ref()
        .and_then(|path| read_first_numbered(path, "power", "_average"))
        .map(|value| value as f64 / 1_000_000.0);
    let fan_rpm = hwmon
        .as_ref()
        .and_then(|path| read_first_numbered(path, "fan", "_input"));
    let fan_percent = hwmon.as_ref().and_then(|path| read_pwm_percent(path));

    Ok(GpuSnapshot {
        id,
        name,
        utilization_percent,
        memory_used_bytes,
        memory_total_bytes,
        temperature_celsius,
        power_watts,
        core_clock_mhz,
        fan_percent,
        fan_rpm,
    })
}

fn first_hwmon(device: &Path) -> Option<PathBuf> {
    let mut entries = fs::read_dir(device.join("hwmon"))
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    entries.sort();
    entries.into_iter().next()
}

fn read_first_numbered(root: &Path, prefix: &str, suffix: &str) -> Option<u64> {
    let mut candidates = fs::read_dir(root)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let number = name
                .strip_prefix(prefix)?
                .strip_suffix(suffix)?
                .parse::<u32>()
                .ok()?;
            Some((number, entry.path()))
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(number, _)| *number);
    candidates
        .into_iter()
        .find_map(|(_, path)| read_optional_u64(path))
}

fn read_pwm_percent(root: &Path) -> Option<f64> {
    let pwm = read_optional_u64(root.join("pwm1"))?;
    let max = read_optional_u64(root.join("pwm1_max"))?;
    (max > 0).then(|| (pwm as f64 / max as f64 * 100.0).clamp(0.0, 100.0))
}

fn read_active_dpm_clock(path: PathBuf) -> Option<u64> {
    let text = fs::read_to_string(path).ok()?;
    text.lines().find_map(|line| {
        if !line.contains('*') {
            return None;
        }
        line.split_whitespace().find_map(|token| {
            let token = token
                .trim_end_matches('*')
                .trim_end_matches("Mhz")
                .trim_end_matches("MHz");
            token.parse::<u64>().ok()
        })
    })
}

fn read_optional_u64(path: PathBuf) -> Option<u64> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn read_device_name(device: &Path) -> Option<String> {
    if let Ok(name) = fs::read_to_string(device.join("product_name")) {
        let name = name.trim();
        if !name.is_empty() {
            return Some(name.to_owned());
        }
    }

    let vendor = fs::read_to_string(device.join("vendor")).ok()?;
    let product = fs::read_to_string(device.join("device")).ok()?;
    Some(format!(
        "AMD GPU {}:{}",
        vendor.trim().trim_start_matches("0x"),
        product.trim().trim_start_matches("0x")
    ))
}

fn merge_sources<T>(left: Collection<Vec<T>>, right: Collection<Vec<T>>) -> Collection<Vec<T>> {
    use Collection::{Available, Degraded, Unavailable};
    match (left, right) {
        (Available(mut left), Available(right)) => {
            left.extend(right);
            Available(left)
        }
        (Available(mut left), Degraded(right))
        | (Degraded(mut left), Available(right))
        | (Degraded(mut left), Degraded(right)) => {
            left.extend(right);
            Degraded(left)
        }
        (Available(value), Unavailable(_))
        | (Degraded(value), Unavailable(_))
        | (Unavailable(_), Available(value))
        | (Unavailable(_), Degraded(value)) => Degraded(value),
        (Unavailable(reason), Unavailable(_)) => Unavailable(reason),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::model::CollectionUnavailable;
    use crate::platform::linux::test_support::Fixture;

    #[test]
    fn merging_available_vendor_sources_preserves_all_devices() {
        let amd = Collection::available(vec![1]);
        let nvidia = Collection::available(vec![2]);
        assert_eq!(
            merge_sources(amd, nvidia),
            Collection::available(vec![1, 2])
        );
    }

    #[test]
    fn one_missing_vendor_is_degraded_not_unavailable() {
        let amd = Collection::available(vec![1]);
        let nvidia = Collection::unavailable(CollectionUnavailable::Unsupported);
        assert_eq!(merge_sources(amd, nvidia), Collection::degraded(vec![1]));
    }

    #[test]
    fn amd_capabilities_map_to_one_gpu_snapshot() {
        let fixture = Fixture::new("amd-gpu");
        fixture.dir("hwmon/hwmon0");
        fixture.write("mem_info_vram_used", "1073741824\n");
        fixture.write("mem_info_vram_total", "8589934592\n");
        fixture.write("gpu_busy_percent", "42\n");
        fixture.write("pp_dpm_sclk", "0: 500Mhz\n1: 2100Mhz *\n");
        fixture.write("product_name", "Radeon Test GPU\n");
        fixture.write("hwmon/hwmon0/temp1_input", "63000\n");
        fixture.write("hwmon/hwmon0/power1_average", "145000000\n");
        fixture.write("hwmon/hwmon0/fan1_input", "1320\n");
        fixture.write("hwmon/hwmon0/pwm1", "94\n");
        fixture.write("hwmon/hwmon0/pwm1_max", "255\n");

        let snapshot = read_amd_device(fixture.path()).unwrap();

        assert_eq!(snapshot.name, "Radeon Test GPU");
        assert_eq!(snapshot.utilization_percent, Some(42.0));
        assert_eq!(snapshot.memory_used_bytes, Some(1_073_741_824));
        assert_eq!(snapshot.memory_total_bytes, Some(8_589_934_592));
        assert_eq!(snapshot.temperature_celsius, Some(63.0));
        assert_eq!(snapshot.power_watts, Some(145.0));
        assert_eq!(snapshot.core_clock_mhz, Some(2100));
        assert_eq!(snapshot.fan_rpm, Some(1320));
        assert!(
            snapshot
                .fan_percent
                .is_some_and(|value| (value - 36.86).abs() < 0.1)
        );
        assert!(snapshot.id.as_opaque_key().starts_with("drm:"));
    }

    #[test]
    fn absent_optional_amd_capabilities_remain_none() {
        let fixture = Fixture::new("amd-gpu-minimal");
        fixture.write("product_name", "Radeon Minimal\n");

        let snapshot = read_amd_device(fixture.path()).unwrap();

        assert_eq!(snapshot.utilization_percent, None);
        assert_eq!(snapshot.memory_used_bytes, None);
        assert_eq!(snapshot.temperature_celsius, None);
        assert_eq!(snapshot.power_watts, None);
        assert_eq!(snapshot.core_clock_mhz, None);
        assert_eq!(snapshot.fan_percent, None);
        assert_eq!(snapshot.fan_rpm, None);
    }
}
