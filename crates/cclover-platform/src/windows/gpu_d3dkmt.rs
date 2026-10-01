#![allow(unsafe_code)]

use std::mem::size_of;

use windows_sys::Wdk::Graphics::Direct3D::{
    D3DKMT_ADAPTER_PERFDATA, D3DKMT_ADAPTERADDRESS, D3DKMT_ADAPTERREGISTRYINFO,
    D3DKMT_CLOSEADAPTER, D3DKMT_ENUMADAPTERS, D3DKMT_PHYSICAL_ADAPTER_COUNT,
    D3DKMT_QUERY_DEVICE_IDS, D3DKMT_QUERYADAPTERINFO, D3DKMTCloseAdapter, D3DKMTEnumAdapters,
    D3DKMTQueryAdapterInfo, KMTQAITYPE_ADAPTERADDRESS, KMTQAITYPE_ADAPTERPERFDATA,
    KMTQAITYPE_ADAPTERREGISTRYINFO, KMTQAITYPE_PHYSICALADAPTERCOUNT,
    KMTQAITYPE_PHYSICALADAPTERDEVICEIDS,
};

const INTEL_VENDOR_ID: u32 = 0x8086;
const NVIDIA_VENDOR_ID: u32 = 0x10DE;
const AMD_VENDOR_ID: u32 = 0x1002;

pub(super) struct Session {
    adapters: Vec<Adapter>,
    devices: Vec<Device>,
    vendors: Vendors,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum VendorPresence {
    Present,
    Absent,
    Unknown,
}

#[derive(Clone, Copy, Default)]
pub(super) struct Vendors {
    intel: bool,
    nvidia: bool,
    amd: bool,
    complete: bool,
}

impl Vendors {
    pub(super) fn intel(self) -> VendorPresence {
        self.presence(self.intel)
    }

    pub(super) fn nvidia(self) -> VendorPresence {
        self.presence(self.nvidia)
    }

    pub(super) fn amd(self) -> VendorPresence {
        self.presence(self.amd)
    }

    fn presence(self, present: bool) -> VendorPresence {
        if present {
            VendorPresence::Present
        } else if self.complete {
            VendorPresence::Absent
        } else {
            VendorPresence::Unknown
        }
    }
}

struct Adapter {
    handle: u32,
}

#[derive(Clone)]
pub(super) struct Device {
    adapter_index: usize,
    physical_index: u32,
    stable_key: String,
    name: String,
}

impl Session {
    pub(super) fn load() -> Result<(Self, Vec<String>), String> {
        let mut enumeration = D3DKMT_ENUMADAPTERS::default();
        // SAFETY: enumeration is writable and initialized according to the D3DKMT ABI.
        let status = unsafe { D3DKMTEnumAdapters(&mut enumeration) };
        if status < 0 {
            return Err(format!(
                "D3DKMTEnumAdapters failed: NTSTATUS 0x{:08x}",
                status as u32
            ));
        }

        let count = (enumeration.NumAdapters as usize).min(enumeration.Adapters.len());
        let mut adapters = Vec::with_capacity(count);
        let mut devices = Vec::new();
        let mut issues = Vec::new();
        let mut vendors = Vendors {
            complete: true,
            ..Default::default()
        };

        for info in enumeration.Adapters[..count].iter() {
            let adapter_index = adapters.len();
            adapters.push(Adapter {
                handle: info.hAdapter,
            });

            let name =
                query::<D3DKMT_ADAPTERREGISTRYINFO>(info.hAdapter, KMTQAITYPE_ADAPTERREGISTRYINFO)
                    .map(|registry| utf16_z(&registry.AdapterString))
                    .unwrap_or_else(|_| "Intel GPU".to_owned());
            let name = if name.is_empty() {
                "Intel GPU".to_owned()
            } else {
                name
            };
            let address =
                query::<D3DKMT_ADAPTERADDRESS>(info.hAdapter, KMTQAITYPE_ADAPTERADDRESS).ok();
            let physical_count = query::<D3DKMT_PHYSICAL_ADAPTER_COUNT>(
                info.hAdapter,
                KMTQAITYPE_PHYSICALADAPTERCOUNT,
            )
            .map(|value| value.Count.max(1))
            .unwrap_or(1);

            for physical_index in 0..physical_count {
                let mut ids = D3DKMT_QUERY_DEVICE_IDS {
                    PhysicalAdapterIndex: physical_index,
                    ..Default::default()
                };
                if let Err(status) =
                    query_into(info.hAdapter, KMTQAITYPE_PHYSICALADAPTERDEVICEIDS, &mut ids)
                {
                    vendors.complete = false;
                    issues.push(format!(
                        "adapter {adapter_index} physical {physical_index} device-id query failed: NTSTATUS 0x{:08x}",
                        status as u32
                    ));
                    continue;
                }
                match ids.DeviceIds.VendorID {
                    INTEL_VENDOR_ID => vendors.intel = true,
                    NVIDIA_VENDOR_ID => vendors.nvidia = true,
                    AMD_VENDOR_ID => vendors.amd = true,
                    _ => {}
                }
                if ids.DeviceIds.VendorID != INTEL_VENDOR_ID {
                    continue;
                }

                let stable_key = match address {
                    Some(address) => format!(
                        "pci:{:04x}:{:04x}:{:04x}:{:02x}:{:02x}.{}:{physical_index}",
                        ids.DeviceIds.VendorID,
                        ids.DeviceIds.DeviceID,
                        ids.DeviceIds.SubSystemID,
                        address.BusNumber,
                        address.DeviceNumber,
                        address.FunctionNumber
                    ),
                    None => format!(
                        "device:{:04x}:{:04x}:{:04x}:{physical_index}",
                        ids.DeviceIds.VendorID, ids.DeviceIds.DeviceID, ids.DeviceIds.SubSystemID
                    ),
                };
                devices.push(Device {
                    adapter_index,
                    physical_index,
                    stable_key,
                    name: if physical_count == 1 {
                        name.clone()
                    } else {
                        format!("{name} #{physical_index}")
                    },
                });
            }
        }

        Ok((
            Self {
                adapters,
                devices,
                vendors,
            },
            issues,
        ))
    }

    pub(super) fn devices(&self) -> &[Device] {
        &self.devices
    }

    pub(super) fn vendors(&self) -> Vendors {
        self.vendors
    }

    pub(super) fn temperature(&self, index: usize) -> Result<f64, i32> {
        let device = self.devices.get(index).ok_or(-1_i32)?;
        let adapter = &self.adapters[device.adapter_index];
        let mut perf = D3DKMT_ADAPTER_PERFDATA {
            PhysicalAdapterIndex: device.physical_index,
            ..Default::default()
        };
        query_into(adapter.handle, KMTQAITYPE_ADAPTERPERFDATA, &mut perf)?;
        let celsius = f64::from(perf.Temperature) / 10.0;
        if (-100.0..=200.0).contains(&celsius) && perf.Temperature != 0 {
            Ok(celsius)
        } else {
            Err(-1)
        }
    }
}

impl Device {
    pub(super) fn stable_key(&self) -> &str {
        &self.stable_key
    }

    pub(super) fn name(&self) -> &str {
        &self.name
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        for adapter in &self.adapters {
            let close = D3DKMT_CLOSEADAPTER {
                hAdapter: adapter.handle,
            };
            // SAFETY: each handle came from D3DKMTEnumAdapters and is closed once with this session.
            let _ = unsafe { D3DKMTCloseAdapter(&close) };
        }
    }
}

fn query<T: Default>(handle: u32, kind: i32) -> Result<T, i32> {
    let mut value = T::default();
    query_into(handle, kind, &mut value)?;
    Ok(value)
}

fn query_into<T>(handle: u32, kind: i32, value: &mut T) -> Result<(), i32> {
    let mut query = D3DKMT_QUERYADAPTERINFO {
        hAdapter: handle,
        Type: kind,
        pPrivateDriverData: (value as *mut T).cast(),
        PrivateDriverDataSize: size_of::<T>() as u32,
    };
    // SAFETY: query points to a live adapter handle and correctly sized writable ABI structure.
    let status = unsafe { D3DKMTQueryAdapterInfo(&mut query) };
    if status < 0 { Err(status) } else { Ok(()) }
}

fn utf16_z(value: &[u16]) -> String {
    let len = value
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(value.len());
    String::from_utf16_lossy(&value[..len]).trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::{VendorPresence, Vendors};

    #[test]
    fn d3dkmt_temperature_unit_is_tenths_celsius() {
        let raw = 573_u32;
        assert!((f64::from(raw) / 10.0 - 57.3).abs() < 1e-9);
    }

    #[test]
    fn incomplete_topology_preserves_unknown_vendor_state() {
        let vendors = Vendors {
            nvidia: true,
            complete: false,
            ..Default::default()
        };
        assert_eq!(vendors.nvidia(), VendorPresence::Present);
        assert_eq!(vendors.amd(), VendorPresence::Unknown);
        assert_eq!(vendors.intel(), VendorPresence::Unknown);
    }

    #[test]
    fn complete_topology_distinguishes_absent_vendor() {
        let vendors = Vendors {
            intel: true,
            complete: true,
            ..Default::default()
        };
        assert_eq!(vendors.intel(), VendorPresence::Present);
        assert_eq!(vendors.nvidia(), VendorPresence::Absent);
        assert_eq!(vendors.amd(), VendorPresence::Absent);
    }
}
