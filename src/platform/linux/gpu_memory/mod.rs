use std::fs;
use std::path::{Path, PathBuf};

use crate::core::model::{Collection, GpuId, GpuMemorySnapshot};

use super::diagnostics::{probe_note, unavailable_from_io};

pub(super) fn collect(
    nvidia: Collection<Vec<GpuMemorySnapshot>>,
    notes: Option<&mut Vec<String>>,
) -> Collection<Vec<GpuMemorySnapshot>> {
    merge_sources(collect_amd(Path::new("/sys/class/drm"), notes), nvidia)
}

fn collect_amd(
    drm_root: &Path,
    mut notes: Option<&mut Vec<String>>,
) -> Collection<Vec<GpuMemorySnapshot>> {
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
                    format!("AMD GPU {name} memory unavailable: {error}")
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

fn read_amd_device(device: &Path) -> Result<GpuMemorySnapshot, String> {
    let used_bytes = read_u64(device.join("mem_info_vram_used"))?;
    let total_bytes = read_u64(device.join("mem_info_vram_total"))?;
    let canonical = fs::canonicalize(device).map_err(|error| error.to_string())?;
    let id = GpuId::from_opaque_key(format!("drm:{}", canonical.display()));
    let name = read_device_name(device).unwrap_or_else(|| "AMD GPU".to_owned());
    Ok(GpuMemorySnapshot {
        id,
        name,
        used_bytes,
        total_bytes,
    })
}

fn read_u64(path: PathBuf) -> Result<u64, String> {
    let value =
        fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    value
        .trim()
        .parse()
        .map_err(|error| format!("{}: {error}", path.display()))
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
    use std::time::{SystemTime, UNIX_EPOCH};

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
    fn amd_vram_files_map_to_gpu_memory_snapshot() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("cclover-mon-amd-vram-{suffix}"));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("mem_info_vram_used"), "1073741824\n").unwrap();
        fs::write(root.join("mem_info_vram_total"), "8589934592\n").unwrap();
        fs::write(root.join("product_name"), "Radeon Test GPU\n").unwrap();

        let snapshot = read_amd_device(&root).unwrap();

        assert_eq!(snapshot.name, "Radeon Test GPU");
        assert_eq!(snapshot.used_bytes, 1_073_741_824);
        assert_eq!(snapshot.total_bytes, 8_589_934_592);
        assert!(snapshot.id.as_opaque_key().starts_with("drm:"));
        fs::remove_dir_all(root).unwrap();
    }
}
