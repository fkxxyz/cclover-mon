mod access;
mod chip;
mod detect;
mod fintek;
mod ite;
mod nuvoton;
mod winbond;

use std::io;

use crate::windows::diagnostics::report_issue;
use crate::windows::pawnio::Session;
use cclover_core::model::{
    Collection, CollectionUnavailable, FanId, FanSnapshot, TemperatureId, TemperatureSnapshot,
};

use access::Access;
use detect::DeviceDescriptor;

pub(super) struct Collector {
    board: Option<super::board::Info>,
    devices: Option<Vec<Device>>,
}

pub(super) struct Observation {
    pub temperatures: Collection<Vec<TemperatureSnapshot>>,
    pub fans: Collection<Vec<FanSnapshot>>,
}

pub(super) enum CollectOutcome {
    Observation(Observation),
    SourceFailed(io::Error),
}

#[derive(Clone, Copy)]
struct Projection {
    temperatures: bool,
    fans: bool,
}

enum Device {
    Fintek(fintek::Reader),
    Ite(ite::Reader),
    Nuvoton(nuvoton::Reader),
    Winbond(winbond::Reader),
}

impl Collector {
    pub(super) fn new(board: Option<super::board::Info>) -> Self {
        Self {
            board,
            devices: None,
        }
    }

    pub(super) fn collect(
        &mut self,
        session: &Session,
        notes: Option<&mut Vec<String>>,
    ) -> CollectOutcome {
        self.collect_projection(
            session,
            notes,
            Projection {
                temperatures: true,
                fans: true,
            },
        )
    }

    pub(super) fn collect_temperatures(
        &mut self,
        session: &Session,
        notes: Option<&mut Vec<String>>,
    ) -> CollectOutcome {
        self.collect_projection(
            session,
            notes,
            Projection {
                temperatures: true,
                fans: false,
            },
        )
    }

    pub(super) fn collect_fans(
        &mut self,
        session: &Session,
        notes: Option<&mut Vec<String>>,
    ) -> CollectOutcome {
        self.collect_projection(
            session,
            notes,
            Projection {
                temperatures: false,
                fans: true,
            },
        )
    }

    fn collect_projection(
        &mut self,
        session: &Session,
        mut notes: Option<&mut Vec<String>>,
        projection: Projection,
    ) -> CollectOutcome {
        let access = Access::new(session);
        let _guard = match Access::lock_isa_bus() {
            Ok(guard) => guard,
            Err(error) => {
                report_issue(&mut notes, || format!("Super-I/O bus unavailable: {error}"));
                return CollectOutcome::Observation(Observation::unavailable(
                    projection,
                    CollectionUnavailable::Unavailable,
                ));
            }
        };

        if self.devices.is_none() {
            match discover_devices(&access, self.board.as_ref(), notes.as_deref_mut()) {
                Ok(devices) => {
                    let count = devices.len();
                    report_issue(&mut notes, || {
                        format!("Super-I/O discovery: devices={count}")
                    });
                    self.devices = Some(devices);
                }
                Err(error) => {
                    report_issue(&mut notes, || {
                        format!("Super-I/O discovery failed: {error}")
                    });
                    return if source_transport_failed(&error) {
                        CollectOutcome::SourceFailed(error)
                    } else {
                        CollectOutcome::Observation(Observation::unavailable(
                            projection,
                            CollectionUnavailable::Unavailable,
                        ))
                    };
                }
            }
        }

        let mut temperatures = Vec::new();
        let mut fans = Vec::new();
        let mut temperature_degraded = false;
        let mut fan_degraded = false;
        for device in self.devices.as_ref().expect("initialized above") {
            if projection.temperatures {
                match device.read_temperatures(&access) {
                    Ok(values) => {
                        let descriptor = device.descriptor();
                        let readable = values.iter().filter(|value| value.is_some()).count();
                        let total = values.len();
                        report_issue(&mut notes, || {
                            format!(
                                "{} temperature channels: readable={readable} total={total}",
                                descriptor.chip.name()
                            )
                        });
                        append_temperatures(&mut temperatures, device, values)
                    }
                    Err(error) => {
                        if source_transport_failed(&error) {
                            report_issue(&mut notes, || {
                                format!("PawnIO Super-I/O session failed: {error}")
                            });
                            return CollectOutcome::SourceFailed(error);
                        }
                        temperature_degraded = true;
                        let descriptor = device.descriptor();
                        report_issue(&mut notes, || {
                            format!(
                                "{} temperature read failed: {error}",
                                descriptor.chip.name()
                            )
                        });
                    }
                }
            }

            if projection.fans {
                match device.read_fans(&access) {
                    Ok(values) => {
                        let descriptor = device.descriptor();
                        let readable = values.iter().filter(|value| value.is_some()).count();
                        let total = values.len();
                        report_issue(&mut notes, || {
                            format!(
                                "{} fan channels: readable={readable} total={total}",
                                descriptor.chip.name()
                            )
                        });
                        append_fans(&mut fans, descriptor, values)
                    }
                    Err(error) => {
                        if source_transport_failed(&error) {
                            report_issue(&mut notes, || {
                                format!("PawnIO Super-I/O session failed: {error}")
                            });
                            return CollectOutcome::SourceFailed(error);
                        }
                        fan_degraded = true;
                        let descriptor = device.descriptor();
                        report_issue(&mut notes, || {
                            format!("{} fan read failed: {error}", descriptor.chip.name())
                        });
                    }
                }
            }
        }

        CollectOutcome::Observation(Observation {
            temperatures: projection_collection(
                projection.temperatures,
                temperature_degraded,
                temperatures,
            ),
            fans: projection_collection(projection.fans, fan_degraded, fans),
        })
    }
}

impl Observation {
    fn unavailable(projection: Projection, reason: CollectionUnavailable) -> Self {
        Self {
            temperatures: if projection.temperatures {
                Collection::unavailable(reason)
            } else {
                Collection::unavailable(CollectionUnavailable::Unsupported)
            },
            fans: if projection.fans {
                Collection::unavailable(reason)
            } else {
                Collection::unavailable(CollectionUnavailable::Unsupported)
            },
        }
    }
}

fn projection_collection<T>(requested: bool, degraded: bool, values: Vec<T>) -> Collection<Vec<T>> {
    if !requested {
        Collection::unavailable(CollectionUnavailable::Unsupported)
    } else if degraded {
        Collection::degraded(values)
    } else {
        Collection::available(values)
    }
}

fn source_transport_failed(error: &io::Error) -> bool {
    error.raw_os_error().is_some()
}

fn discover_devices(
    access: &Access<'_>,
    board: Option<&super::board::Info>,
    mut notes: Option<&mut Vec<String>>,
) -> io::Result<Vec<Device>> {
    let mut devices = Vec::new();
    let discovery = detect::discover(access, board)?;
    for detail in discovery.details {
        report_issue(&mut notes, || format!("Super-I/O probe: {detail}"));
    }
    for descriptor in discovery.devices {
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

fn append_temperatures(
    output: &mut Vec<TemperatureSnapshot>,
    device: &Device,
    values: Vec<Option<f64>>,
) {
    let descriptor = device.descriptor();
    if let Device::Nuvoton(reader) = device {
        for ((key, name), celsius) in reader.temperature_metadata().zip(values) {
            let Some(celsius) = celsius else {
                continue;
            };
            output.push(TemperatureSnapshot {
                id: TemperatureId::from_opaque_key(format!(
                    "windows:superio:slot{}:{}:temp:{key}",
                    descriptor.slot,
                    descriptor.chip.stable_key(),
                )),
                name: name.to_owned(),
                celsius,
            });
        }
        return;
    }

    for (index, celsius) in values.into_iter().enumerate() {
        let Some(celsius) = celsius else {
            continue;
        };
        output.push(TemperatureSnapshot {
            id: TemperatureId::from_opaque_key(format!(
                "windows:superio:slot{}:{}:temp{}",
                descriptor.slot,
                descriptor.chip.stable_key(),
                index + 1
            )),
            name: format!("{} Temperature #{}", descriptor.chip.name(), index + 1),
            celsius,
        });
    }
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

    fn read_temperatures(&self, access: &Access<'_>) -> io::Result<Vec<Option<f64>>> {
        match self {
            Self::Fintek(reader) => reader.read_temperatures(access),
            Self::Ite(reader) => reader.read_temperatures(access),
            Self::Nuvoton(reader) => reader.read_temperatures(access),
            Self::Winbond(reader) => reader.read_temperatures(access),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_collection_preserves_partial_success() {
        assert!(matches!(
            projection_collection(true, false, vec![1_u8]),
            Collection::Available(values) if values == vec![1]
        ));
        assert!(matches!(
            projection_collection(true, true, vec![1_u8]),
            Collection::Degraded(values) if values == vec![1]
        ));
        assert!(matches!(
            projection_collection::<u8>(false, false, Vec::new()),
            Collection::Unavailable(CollectionUnavailable::Unsupported)
        ));
    }
}
