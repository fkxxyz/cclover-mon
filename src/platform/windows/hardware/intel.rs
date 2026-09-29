// Intel package-temperature semantics are checked against LibreHardwareMonitor's
// reviewed hardware implementation, but this file uses the project-owned PawnIO client.

use std::io;

use crate::core::model::TemperatureSnapshot;

use super::super::pawnio::Session;

const IA32_TEMPERATURE_TARGET: u64 = 0x1A2;
const IA32_PACKAGE_THERM_STATUS: u64 = 0x1B1;

pub(super) fn collect(session: &Session) -> io::Result<TemperatureSnapshot> {
    let target = read_msr(session, IA32_TEMPERATURE_TARGET)?;
    let status = read_msr(session, IA32_PACKAGE_THERM_STATUS)?;
    if status & (1 << 31) == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "IA32_PACKAGE_THERM_STATUS is not valid",
        ));
    }

    let tj_max = ((target >> 16) & 0xff) as f64;
    let delta = ((status >> 16) & 0x7f) as f64;
    let celsius = tj_max - delta;
    if !(0.0..=125.0).contains(&celsius) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("implausible Intel package temperature {celsius:.1}C"),
        ));
    }

    Ok(TemperatureSnapshot {
        id: "windows:intel-package".to_owned(),
        name: "CPU Package".to_owned(),
        celsius,
    })
}

fn read_msr(session: &Session, msr: u64) -> io::Result<u64> {
    session
        .execute("ioctl_read_msr", &[msr], 1)?
        .first()
        .copied()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "PawnIO MSR read returned no value",
            )
        })
}

#[cfg(test)]
mod tests {
    #[test]
    fn package_temperature_decodes_tjmax_minus_delta() {
        let target = 100_u64 << 16;
        let status = (1_u64 << 31) | (37_u64 << 16);
        let tj_max = ((target >> 16) & 0xff) as f64;
        let delta = ((status >> 16) & 0x7f) as f64;
        assert_eq!(tj_max - delta, 63.0);
    }
}
