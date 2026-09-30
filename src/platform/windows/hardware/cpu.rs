#![allow(unsafe_code)]

use std::io;
use std::mem::{offset_of, size_of};
use std::ptr::{null_mut, read_unaligned};

use windows_sys::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER;
use windows_sys::Win32::System::SystemInformation::{
    GROUP_AFFINITY, GetLogicalProcessorInformationEx, PROCESSOR_RELATIONSHIP,
    RelationProcessorCore, SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX,
};
use windows_sys::Win32::System::Threading::{GetCurrentThread, SetThreadGroupAffinity};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Vendor {
    Intel,
    Amd,
    Other,
}

#[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))]
#[derive(Clone, Debug)]
pub(super) struct Info {
    pub vendor: Vendor,
    pub family: u32,
    pub model: u32,
    pub stepping: u32,
    pub brand: String,
    pub core_dts: bool,
    pub package_dts: bool,
    pub cpuid_80000001_ebx: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CoreAffinity {
    pub group: u16,
    pub mask: usize,
}

pub(super) struct AffinityGuard {
    previous: GROUP_AFFINITY,
}

pub(super) fn detect() -> Info {
    let leaf0 = cpuid(0);
    let vendor_bytes = [
        leaf0.ebx.to_le_bytes(),
        leaf0.edx.to_le_bytes(),
        leaf0.ecx.to_le_bytes(),
    ]
    .concat();
    let vendor = match vendor_bytes.as_slice() {
        b"GenuineIntel" => Vendor::Intel,
        b"AuthenticAMD" => Vendor::Amd,
        _ => Vendor::Other,
    };

    let leaf1 = cpuid(1);
    let base_family = (leaf1.eax >> 8) & 0x0F;
    let extended_family = (leaf1.eax >> 20) & 0xFF;
    let family = if base_family == 0x0F {
        base_family + extended_family
    } else {
        base_family
    };
    let stepping = leaf1.eax & 0x0F;
    let base_model = (leaf1.eax >> 4) & 0x0F;
    let extended_model = (leaf1.eax >> 16) & 0x0F;
    let model = if matches!(base_family, 0x06 | 0x0F) {
        base_model | (extended_model << 4)
    } else {
        base_model
    };

    let leaf6 = if leaf0.eax >= 6 { Some(cpuid(6)) } else { None };
    let max_extended = cpuid(0x8000_0000).eax;
    let cpuid_80000001_ebx = if max_extended >= 0x8000_0001 {
        cpuid(0x8000_0001).ebx
    } else {
        0
    };
    Info {
        vendor,
        family,
        model,
        stepping,
        brand: cpu_brand(),
        core_dts: leaf6.is_some_and(|leaf| leaf.eax & 1 != 0),
        package_dts: leaf6.is_some_and(|leaf| leaf.eax & (1 << 6) != 0),
        cpuid_80000001_ebx,
    }
}

pub(super) fn physical_core_affinities() -> io::Result<Vec<CoreAffinity>> {
    let mut bytes = 0_u32;
    // SAFETY: documented sizing call; null buffer with zero size requests required length.
    let first =
        unsafe { GetLogicalProcessorInformationEx(RelationProcessorCore, null_mut(), &mut bytes) };
    if first != 0
        || io::Error::last_os_error().raw_os_error() != Some(ERROR_INSUFFICIENT_BUFFER as i32)
    {
        return if first != 0 {
            Ok(Vec::new())
        } else {
            Err(io::Error::last_os_error())
        };
    }
    if bytes == 0 {
        return Ok(Vec::new());
    }

    let word = size_of::<usize>();
    let mut storage = vec![0_usize; (bytes as usize).div_ceil(word)];
    let mut actual = bytes;
    // SAFETY: storage is writable and aligned at least to usize; capacity covers actual bytes.
    if unsafe {
        GetLogicalProcessorInformationEx(
            RelationProcessorCore,
            storage.as_mut_ptr().cast(),
            &mut actual,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }

    let bytes =
        unsafe { std::slice::from_raw_parts(storage.as_ptr().cast::<u8>(), actual as usize) };
    parse_physical_core_affinities(bytes)
}

fn parse_physical_core_affinities(bytes: &[u8]) -> io::Result<Vec<CoreAffinity>> {
    const HEADER_SIZE: usize = 8;
    const RELATIONSHIP_OFFSET: usize = 0;
    const SIZE_OFFSET: usize = 4;
    const PROCESSOR_OFFSET: usize = offset_of!(SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX, Anonymous);
    const GROUP_COUNT_OFFSET: usize =
        PROCESSOR_OFFSET + offset_of!(PROCESSOR_RELATIONSHIP, GroupCount);
    const GROUP_MASK_OFFSET: usize =
        PROCESSOR_OFFSET + offset_of!(PROCESSOR_RELATIONSHIP, GroupMask);

    let mut cores = Vec::new();
    let mut offset = 0_usize;
    while offset < bytes.len() {
        if bytes.len() - offset < HEADER_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "truncated logical processor topology header",
            ));
        }

        let relationship = i32::from_ne_bytes(
            bytes[offset + RELATIONSHIP_OFFSET..offset + RELATIONSHIP_OFFSET + 4]
                .try_into()
                .expect("fixed-width topology relationship field"),
        );
        let size = u32::from_ne_bytes(
            bytes[offset + SIZE_OFFSET..offset + SIZE_OFFSET + 4]
                .try_into()
                .expect("fixed-width topology size field"),
        ) as usize;
        if size < HEADER_SIZE || offset.saturating_add(size) > bytes.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid logical processor topology entry size",
            ));
        }

        if relationship == RelationProcessorCore {
            if size < GROUP_MASK_OFFSET + size_of::<GROUP_AFFINITY>() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "truncated processor relationship",
                ));
            }
            let group_count = u16::from_ne_bytes(
                bytes[offset + GROUP_COUNT_OFFSET..offset + GROUP_COUNT_OFFSET + 2]
                    .try_into()
                    .expect("fixed-width processor group count"),
            );
            for index in 0..usize::from(group_count) {
                let relative_mask_offset = GROUP_MASK_OFFSET + index * size_of::<GROUP_AFFINITY>();
                if relative_mask_offset + size_of::<GROUP_AFFINITY>() > size {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "truncated processor group affinity",
                    ));
                }
                // SAFETY: bounds checked against the current variable-length record.
                let group = unsafe {
                    read_unaligned(
                        bytes
                            .as_ptr()
                            .add(offset + relative_mask_offset)
                            .cast::<GROUP_AFFINITY>(),
                    )
                };
                if group.Mask != 0 {
                    cores.push(CoreAffinity {
                        group: group.Group,
                        mask: group.Mask & group.Mask.wrapping_neg(),
                    });
                    break;
                }
            }
        }
        offset += size;
    }
    Ok(cores)
}

impl AffinityGuard {
    pub(super) fn bind(target: CoreAffinity) -> io::Result<Self> {
        let affinity = GROUP_AFFINITY {
            Mask: target.mask,
            Group: target.group,
            Reserved: [0; 3],
        };
        let mut previous = GROUP_AFFINITY::default();
        // SAFETY: pseudo handle is valid for current thread; affinity/previous are valid structs.
        if unsafe { SetThreadGroupAffinity(GetCurrentThread(), &affinity, &mut previous) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { previous })
    }
}

impl Drop for AffinityGuard {
    fn drop(&mut self) {
        // SAFETY: previous was returned by successful SetThreadGroupAffinity for this thread.
        unsafe {
            SetThreadGroupAffinity(GetCurrentThread(), &self.previous, null_mut());
        }
    }
}

fn cpu_brand() -> String {
    let max = cpuid(0x8000_0000).eax;
    if max < 0x8000_0004 {
        return String::new();
    }
    let mut bytes = Vec::with_capacity(48);
    for leaf in 0x8000_0002..=0x8000_0004 {
        let value = cpuid(leaf);
        bytes.extend_from_slice(&value.eax.to_le_bytes());
        bytes.extend_from_slice(&value.ebx.to_le_bytes());
        bytes.extend_from_slice(&value.ecx.to_le_bytes());
        bytes.extend_from_slice(&value.edx.to_le_bytes());
    }
    String::from_utf8_lossy(&bytes)
        .trim_matches(char::from(0))
        .trim()
        .to_owned()
}

#[derive(Clone, Copy)]
struct CpuidResult {
    eax: u32,
    ebx: u32,
    ecx: u32,
    edx: u32,
}

#[cfg(target_arch = "x86_64")]
fn cpuid(leaf: u32) -> CpuidResult {
    let value = std::arch::x86_64::__cpuid(leaf);
    CpuidResult {
        eax: value.eax,
        ebx: value.ebx,
        ecx: value.ecx,
        edx: value.edx,
    }
}

#[cfg(target_arch = "x86")]
fn cpuid(leaf: u32) -> CpuidResult {
    let value = std::arch::x86::__cpuid(leaf);
    CpuidResult {
        eax: value.eax,
        ebx: value.ebx,
        ecx: value.ecx,
        edx: value.edx,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_cpu_detection_has_a_known_vendor_or_explicit_other() {
        assert!(matches!(
            detect().vendor,
            Vendor::Intel | Vendor::Amd | Vendor::Other
        ));
    }

    #[test]
    fn local_physical_core_topology_is_nonempty() {
        assert!(!physical_core_affinities().unwrap().is_empty());
    }

    #[test]
    fn processor_core_parser_accepts_variable_length_records_smaller_than_union_wrapper() {
        const PROCESSOR_OFFSET: usize =
            offset_of!(SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX, Anonymous);
        const GROUP_COUNT_OFFSET: usize =
            PROCESSOR_OFFSET + offset_of!(PROCESSOR_RELATIONSHIP, GroupCount);
        const GROUP_MASK_OFFSET: usize =
            PROCESSOR_OFFSET + offset_of!(PROCESSOR_RELATIONSHIP, GroupMask);
        let record_size = GROUP_MASK_OFFSET + size_of::<GROUP_AFFINITY>();
        assert!(record_size < size_of::<SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX>());

        let mut record = vec![0_u8; record_size];
        record[..4].copy_from_slice(&RelationProcessorCore.to_ne_bytes());
        record[4..8].copy_from_slice(&(record_size as u32).to_ne_bytes());
        record[GROUP_COUNT_OFFSET..GROUP_COUNT_OFFSET + 2].copy_from_slice(&1_u16.to_ne_bytes());
        let affinity = GROUP_AFFINITY {
            Mask: 0b1010,
            Group: 3,
            Reserved: [0; 3],
        };
        // SAFETY: record reserves exactly one complete GROUP_AFFINITY at this offset.
        unsafe {
            std::ptr::write_unaligned(
                record
                    .as_mut_ptr()
                    .add(GROUP_MASK_OFFSET)
                    .cast::<GROUP_AFFINITY>(),
                affinity,
            );
        }

        assert_eq!(
            parse_physical_core_affinities(&record).unwrap(),
            vec![CoreAffinity {
                group: 3,
                mask: 0b0010,
            }]
        );
    }

    #[test]
    fn processor_core_parser_rejects_truncated_group_affinity() {
        const PROCESSOR_OFFSET: usize =
            offset_of!(SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX, Anonymous);
        const GROUP_COUNT_OFFSET: usize =
            PROCESSOR_OFFSET + offset_of!(PROCESSOR_RELATIONSHIP, GroupCount);
        const GROUP_MASK_OFFSET: usize =
            PROCESSOR_OFFSET + offset_of!(PROCESSOR_RELATIONSHIP, GroupMask);
        let mut record = vec![0_u8; GROUP_MASK_OFFSET + size_of::<GROUP_AFFINITY>()];
        let record_size = record.len() as u32;
        record[..4].copy_from_slice(&RelationProcessorCore.to_ne_bytes());
        record[4..8].copy_from_slice(&record_size.to_ne_bytes());
        record[GROUP_COUNT_OFFSET..GROUP_COUNT_OFFSET + 2].copy_from_slice(&2_u16.to_ne_bytes());

        assert_eq!(
            parse_physical_core_affinities(&record).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }
}
