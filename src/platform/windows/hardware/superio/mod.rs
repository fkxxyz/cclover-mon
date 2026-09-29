mod access;
mod chip;
mod detect;
mod fintek;
mod ite;
mod nuvoton;
mod winbond;

use std::io;

use crate::core::model::{Collection, CollectionUnavailable, FanId, FanSnapshot};
use crate::platform::windows::diagnostics::report_issue;
use crate::platform::windows::pawnio::Session;

use access::Access;
use detect::DeviceDescriptor;

pub(super) struct Collector {
    devices: Option<Vec<Device>>,
}

pub(super) enum CollectOutcome {
    Observation(Collection<Vec<FanSnapshot>>),
    SourceFailed(io::Error),
}

enum Device {
    Fintek(fintek::Reader),
    Ite(ite::Reader),
    Nuvoton(nuvoton::Reader),
    Winbond(winbond::Reader),
}

impl Collector {
    pub(super) fn new() -> Self {
        Self { devices: None }
    }

    pub(super) fn collect(
        &mut self,
        session: &Session,
        mut notes: Option<&mut Vec<String>>,
    ) -> CollectOutcome {
        let access = Access::new(session);
        let _guard = match Access::lock_isa_bus() {
            Ok(guard) => guard,
            Err(error) => {
                report_issue(&mut notes, || format!("Super-I/O bus unavailable: {error}"));
                return CollectOutcome::Observation(Collection::unavailable(
                    CollectionUnavailable::Unavailable,
                ));
            }
        };

        if self.devices.is_none() {
            match discover_devices(&access, notes.as_deref_mut()) {
                Ok(devices) => self.devices = Some(devices),
                Err(error) => {
                    report_issue(&mut notes, || {
                        format!("Super-I/O discovery failed: {error}")
                    });
                    return if source_transport_failed(&error) {
                        CollectOutcome::SourceFailed(error)
                    } else {
                        CollectOutcome::Observation(Collection::unavailable(
                            CollectionUnavailable::Unavailable,
                        ))
                    };
                }
            }
        }

        let mut fans = Vec::new();
        let mut degraded = false;
        for device in self.devices.as_ref().expect("initialized above") {
            match device.read_fans(&access) {
                Ok(values) => append_fans(&mut fans, device.descriptor(), values),
                Err(error) => {
                    if source_transport_failed(&error) {
                        report_issue(&mut notes, || {
                            format!("PawnIO Super-I/O session failed: {error}")
                        });
                        return CollectOutcome::SourceFailed(error);
                    }
                    degraded = true;
                    let descriptor = device.descriptor();
                    report_issue(&mut notes, || {
                        format!("{} fan read failed: {error}", descriptor.chip.name())
                    });
                }
            }
        }

        if degraded {
            CollectOutcome::Observation(Collection::degraded(fans))
        } else {
            CollectOutcome::Observation(Collection::available(fans))
        }
    }
}

fn source_transport_failed(error: &io::Error) -> bool {
    error.raw_os_error().is_some()
}

fn discover_devices(
    access: &Access<'_>,
    mut notes: Option<&mut Vec<String>>,
) -> io::Result<Vec<Device>> {
    let mut devices = Vec::new();
    for descriptor in detect::discover(access)? {
        let device = if descriptor.chip.is_fintek() {
            Some(Device::Fintek(fintek::Reader::new(descriptor)))
        } else if descriptor.chip.is_ite() {
            ite::Reader::new(descriptor, access)?.map(Device::Ite)
        } else if descriptor.chip.is_winbond() {
            winbond::Reader::new(descriptor, access)?.map(Device::Winbond)
        } else {
            nuvoton::Reader::new(descriptor, access)?.map(Device::Nuvoton)
        };

        if let Some(device) = device {
            devices.push(device);
        } else {
            report_issue(&mut notes, || {
                format!(
                    "{} detected but rejected by family validation",
                    descriptor.chip.name()
                )
            });
        }
    }
    Ok(devices)
}

fn append_fans(
    output: &mut Vec<FanSnapshot>,
    descriptor: DeviceDescriptor,
    values: Vec<Option<u64>>,
) {
    for (index, rpm) in values.into_iter().enumerate() {
        let Some(rpm) = rpm else {
            continue;
        };
        output.push(FanSnapshot {
            id: FanId::from_opaque_key(format!(
                "windows:superio:slot{}:{}:fan{}",
                descriptor.slot,
                descriptor.chip.stable_key(),
                index + 1
            )),
            name: format!("{} Fan #{}", descriptor.chip.name(), index + 1),
            rpm,
        });
    }
}

impl Device {
    fn descriptor(&self) -> DeviceDescriptor {
        match self {
            Self::Fintek(reader) => reader.descriptor(),
            Self::Ite(reader) => reader.descriptor(),
            Self::Nuvoton(reader) => reader.descriptor(),
            Self::Winbond(reader) => reader.descriptor(),
        }
    }

    fn read_fans(&self, access: &Access<'_>) -> io::Result<Vec<Option<u64>>> {
        match self {
            Self::Fintek(reader) => reader.read_fans(access),
            Self::Ite(reader) => reader.read_fans(access),
            Self::Nuvoton(reader) => reader.read_fans(access),
            Self::Winbond(reader) => reader.read_fans(access),
        }
    }
}
