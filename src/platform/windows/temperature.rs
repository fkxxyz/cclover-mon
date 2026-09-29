use std::time::{Duration, Instant};

use crate::core::model::{Collection, CollectionUnavailable, TemperatureSnapshot};

use super::diagnostics::report_issue;
use super::pawnio::{OpenError, Session};
use super::provision::{self, MachineStatus};

const INTEL_MSR_MODULE: &[u8] = include_bytes!(env!("CCLOVER_PAWNIO_INTEL_MSR_BIN"));
const IA32_TEMPERATURE_TARGET: u64 = 0x1A2;
const IA32_PACKAGE_THERM_STATUS: u64 = 0x1B1;
const RETRY_INTERVAL: Duration = Duration::from_secs(30);

pub(super) struct Collector {
    state: State,
}

enum State {
    Uninitialized,
    Ready(IntelPackageTemperature),
    Retry {
        at: Instant,
        reason: CollectionUnavailable,
    },
    Unsupported,
    StableFailure(CollectionUnavailable),
}

struct IntelPackageTemperature {
    session: Session,
}

impl Collector {
    pub(super) fn new() -> Self {
        Self {
            state: State::Uninitialized,
        }
    }

    pub(super) fn collect(
        &mut self,
        mut notes: Option<&mut Vec<String>>,
    ) -> Collection<Vec<TemperatureSnapshot>> {
        if matches!(self.state, State::Uninitialized) {
            self.state = initialize_state();
        } else if let State::Retry { at, .. } = self.state
            && Instant::now() >= at
        {
            self.state = initialize_state();
        }

        match &mut self.state {
            State::Uninitialized => unreachable!("temperature collector initialized above"),
            State::Ready(source) => match source.read_package_temperature() {
                Ok(celsius) => Collection::available(vec![TemperatureSnapshot {
                    id: "windows:intel-package".to_owned(),
                    name: "CPU Package".to_owned(),
                    celsius,
                }]),
                Err(error) => {
                    report_issue(&mut notes, || {
                        format!("PawnIO Intel package temperature failed: {error}")
                    });
                    self.state = State::Retry {
                        at: Instant::now() + RETRY_INTERVAL,
                        reason: CollectionUnavailable::Unavailable,
                    };
                    Collection::unavailable(CollectionUnavailable::Unavailable)
                }
            },
            State::Retry { reason, .. } => Collection::unavailable(*reason),
            State::Unsupported => Collection::unavailable(CollectionUnavailable::Unsupported),
            State::StableFailure(reason) => Collection::unavailable(*reason),
        }
    }
}

impl Default for Collector {
    fn default() -> Self {
        Self::new()
    }
}

impl IntelPackageTemperature {
    fn read_package_temperature(&self) -> std::io::Result<f64> {
        let target = self.read_msr(IA32_TEMPERATURE_TARGET)?;
        let status = self.read_msr(IA32_PACKAGE_THERM_STATUS)?;
        if status & (1 << 31) == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "IA32_PACKAGE_THERM_STATUS is not valid",
            ));
        }
        let tj_max = ((target >> 16) & 0xff) as f64;
        let delta = ((status >> 16) & 0x7f) as f64;
        let celsius = tj_max - delta;
        if !(0.0..=125.0).contains(&celsius) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("implausible Intel package temperature {celsius:.1}C"),
            ));
        }
        Ok(celsius)
    }

    fn read_msr(&self, msr: u64) -> std::io::Result<u64> {
        let output = self.session.execute("ioctl_read_msr", &[msr], 1)?;
        output.first().copied().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "PawnIO MSR read returned no value",
            )
        })
    }
}

#[derive(Debug)]
enum InitFailure {
    Unsupported,
    Retry(CollectionUnavailable),
    Stable(CollectionUnavailable),
}

impl InitFailure {
    fn into_state(self) -> State {
        match self {
            Self::Unsupported => State::Unsupported,
            Self::Retry(reason) => State::Retry {
                at: Instant::now() + RETRY_INTERVAL,
                reason,
            },
            Self::Stable(reason) => State::StableFailure(reason),
        }
    }
}

fn initialize() -> Result<IntelPackageTemperature, InitFailure> {
    let session = match Session::open() {
        Ok(session) => session,
        Err(OpenError::NotInstalled) => match provision::machine_status() {
            Some(MachineStatus::Ready) => Session::open()
                .map_err(|_| InitFailure::Retry(CollectionUnavailable::Unavailable))?,
            Some(MachineStatus::Unsupported) => return Err(InitFailure::Unsupported),
            #[cfg(not(target_arch = "x86_64"))]
            Some(MachineStatus::ProvisioningUnsupported) => return Err(InitFailure::Unsupported),
            #[cfg(target_arch = "x86_64")]
            Some(MachineStatus::ElevationDeclined) => {
                return Err(InitFailure::Stable(CollectionUnavailable::PermissionDenied));
            }
            #[cfg(target_arch = "x86_64")]
            Some(MachineStatus::RebootRequired) => {
                return Err(InitFailure::Stable(CollectionUnavailable::Unavailable));
            }
            Some(MachineStatus::InstallFailed) | None => {
                return Err(InitFailure::Stable(CollectionUnavailable::Unavailable));
            }
        },
        Err(OpenError::PermissionDenied) => {
            return Err(InitFailure::Retry(CollectionUnavailable::PermissionDenied));
        }
        Err(OpenError::Other) => {
            return Err(InitFailure::Retry(CollectionUnavailable::Unavailable));
        }
    };
    if let Err(error) = session.load_module(INTEL_MSR_MODULE) {
        return match error.raw_os_error() {
            Some(50) => Err(InitFailure::Unsupported),
            Some(5) => Err(InitFailure::Retry(CollectionUnavailable::PermissionDenied)),
            _ => Err(InitFailure::Retry(CollectionUnavailable::Unavailable)),
        };
    }
    Ok(IntelPackageTemperature { session })
}

fn initialize_state() -> State {
    match initialize() {
        Ok(source) => State::Ready(source),
        Err(failure) => failure.into_state(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn intel_package_temperature_decodes_tjmax_minus_delta() {
        let target = 100_u64 << 16;
        let status = (1_u64 << 31) | (37_u64 << 16);
        let tj_max = ((target >> 16) & 0xff) as f64;
        let delta = ((status >> 16) & 0x7f) as f64;
        assert_eq!(tj_max - delta, 63.0);
    }
}
