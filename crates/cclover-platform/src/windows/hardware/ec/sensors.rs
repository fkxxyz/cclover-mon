use super::*;

pub(super) fn sensor_spec(family: Family, sensor: Sensor) -> Option<SensorSpec> {
    let temperature = |key, name, register, blank| SensorSpec {
        key,
        name,
        kind: Kind::Temperature,
        register,
        size: 1,
        blank,
    };
    let fan = |key, name, register| SensorSpec {
        key,
        name,
        kind: Kind::Fan,
        register,
        size: 2,
        blank: None,
    };

    match (family, sensor) {
        (Family::Amd400, Sensor::TempChipset) | (Family::Amd500, Sensor::TempChipset) => {
            Some(temperature("chipset", "Chipset", 0x003A, None))
        }
        (Family::Amd400, Sensor::TempCPU) | (Family::Amd500, Sensor::TempCPU) => {
            Some(temperature("cpu", "CPU", 0x003B, None))
        }
        (Family::Amd400, Sensor::TempMB) | (Family::Amd500, Sensor::TempMB) => {
            Some(temperature("motherboard", "Motherboard", 0x003C, None))
        }
        (Family::Amd400, Sensor::TempTSensor) | (Family::Amd500, Sensor::TempTSensor) => {
            Some(temperature("t-sensor", "T Sensor", 0x003D, Some(-40)))
        }
        (Family::Amd400, Sensor::TempVrm) | (Family::Amd500, Sensor::TempVrm) => {
            Some(temperature("vrm", "VRM", 0x003E, None))
        }
        (Family::Amd400, Sensor::FanCPUOpt) => Some(fan("cpu-opt-fan", "CPU Optional Fan", 0x00BC)),
        (Family::Amd500, Sensor::FanCPUOpt)
        | (Family::Amd600, Sensor::FanCPUOpt)
        | (Family::Amd800, Sensor::FanCPUOpt) => {
            Some(fan("cpu-opt-fan", "CPU Optional Fan", 0x00B0))
        }
        (Family::Amd400, Sensor::FanVrmHS) | (Family::Amd500, Sensor::FanVrmHS) => {
            Some(fan("vrm-heatsink-fan", "VRM Heat Sink Fan", 0x00B2))
        }
        (Family::Amd500, Sensor::FanChipset) => Some(fan("chipset-fan", "Chipset Fan", 0x00B4)),
        (Family::Amd400, Sensor::TempWaterIn) => {
            Some(temperature("water-in", "Water In", 0x010D, Some(-40)))
        }
        (Family::Amd400, Sensor::TempWaterOut) => {
            Some(temperature("water-out", "Water Out", 0x010B, Some(-40)))
        }
        (Family::Amd500, Sensor::TempWaterIn) | (Family::Amd600, Sensor::TempWaterIn) => {
            Some(temperature("water-in", "Water In", 0x0100, Some(-40)))
        }
        (Family::Amd500, Sensor::TempWaterOut) | (Family::Amd600, Sensor::TempWaterOut) => {
            Some(temperature("water-out", "Water Out", 0x0101, Some(-40)))
        }
        (Family::Amd800, Sensor::TempCPU) => Some(temperature("cpu", "CPU", 0x0030, None)),
        (Family::Amd800, Sensor::TempCPUPackage) => {
            Some(temperature("cpu-package", "CPU Package", 0x0031, None))
        }
        (Family::Amd800, Sensor::TempMB) => {
            Some(temperature("motherboard", "Motherboard", 0x0032, None))
        }
        (Family::Amd800, Sensor::TempVrm) => Some(temperature("vrm", "VRM", 0x0033, None)),
        (Family::Amd800, Sensor::TempTSensorAlt) => {
            Some(temperature("t-sensor", "T Sensor", 0x0035, Some(-40)))
        }
        (Family::Amd800, Sensor::TempTSensor) => {
            Some(temperature("t-sensor", "T Sensor", 0x0036, Some(-40)))
        }
        (Family::Intel100, Sensor::TempChipset)
        | (Family::Intel300, Sensor::TempChipset)
        | (Family::Intel370, Sensor::TempChipset)
        | (Family::Intel400, Sensor::TempChipset) => {
            Some(temperature("chipset", "Chipset", 0x003A, None))
        }
        (Family::Intel100, Sensor::TempVrm)
        | (Family::Intel300, Sensor::TempVrm)
        | (Family::Intel400, Sensor::TempVrm)
        | (Family::Intel600, Sensor::TempVrm) => Some(temperature("vrm", "VRM", 0x003E, None)),
        (Family::Intel100, Sensor::TempTSensor)
        | (Family::Intel300, Sensor::TempTSensor)
        | (Family::Intel370, Sensor::TempTSensor)
        | (Family::Intel400, Sensor::TempTSensor)
        | (Family::Intel600, Sensor::TempTSensor) => {
            Some(temperature("t-sensor", "T Sensor", 0x003D, Some(-40)))
        }
        (Family::Intel300, Sensor::TempWaterIn)
        | (Family::Intel400, Sensor::TempWaterIn)
        | (Family::Intel600, Sensor::TempWaterIn) => {
            Some(temperature("water-in", "Water In", 0x0100, Some(-40)))
        }
        (Family::Intel300, Sensor::TempWaterOut)
        | (Family::Intel400, Sensor::TempWaterOut)
        | (Family::Intel600, Sensor::TempWaterOut) => {
            Some(temperature("water-out", "Water Out", 0x0101, Some(-40)))
        }
        (Family::Intel600, Sensor::TempWaterBlockIn) => Some(temperature(
            "water-block-in",
            "Water Block In",
            0x0102,
            Some(-40),
        )),
        (Family::Intel300, Sensor::FanCPUOpt) | (Family::Intel400, Sensor::FanCPUOpt) => {
            Some(fan("cpu-opt-fan", "CPU Optional Fan", 0x00B0))
        }
        (Family::Intel370, Sensor::FanCPUOpt) => {
            Some(fan("cpu-opt-fan", "CPU Optional Fan", 0x00BC))
        }
        (Family::Intel100, Sensor::FanWaterPump) => Some(fan("water-pump", "Water Pump", 0x00BC)),
        (Family::Intel370, Sensor::FanWaterPump) => Some(fan("water-pump", "Water Pump", 0x00BE)),
        (Family::Intel700, Sensor::TempVrm) | (Family::Intel800, Sensor::TempVrm) => {
            Some(temperature("vrm", "VRM", 0x0033, None))
        }
        (Family::Intel700, Sensor::FanCPUOpt) => {
            Some(fan("cpu-opt-fan", "CPU Optional Fan", 0x00B0))
        }
        (Family::Intel700, Sensor::TempTSensor) => {
            Some(temperature("t-sensor", "T Sensor", 0x0109, Some(-40)))
        }
        (Family::Intel700, Sensor::TempTSensor2) => {
            Some(temperature("t-sensor-2", "T Sensor 2", 0x0105, Some(-40)))
        }
        (Family::Intel700, Sensor::TempWaterIn) => {
            Some(temperature("water-in", "Water In", 0x0100, Some(-40)))
        }
        (Family::Intel700, Sensor::TempWaterOut) => {
            Some(temperature("water-out", "Water Out", 0x0101, Some(-40)))
        }
        (Family::Intel800, Sensor::TempTSensor) => {
            Some(temperature("t-sensor", "T Sensor", 0x010C, Some(-40)))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reviewed_board_table_is_exact_and_unknown_boards_are_not_probed() {
        let known = board_config("rogcrosshairx870ehero").unwrap();
        assert!(matches!(known.family, Family::Amd800));
        assert!(
            known
                .sensors
                .iter()
                .any(|sensor| matches!(sensor, Sensor::TempVrm))
        );
        assert!(board_config("unknownboard").is_none());
    }

    #[test]
    fn reviewed_family_registers_keep_blank_temperature_semantics() {
        let spec = sensor_spec(Family::Intel700, Sensor::TempTSensor).unwrap();
        assert_eq!(spec.register, 0x0109);
        assert_eq!(spec.blank, Some(-40));
        let fan = sensor_spec(Family::Amd500, Sensor::FanChipset).unwrap();
        assert_eq!(fan.register, 0x00B4);
        assert_eq!(fan.size, 2);
    }
}
