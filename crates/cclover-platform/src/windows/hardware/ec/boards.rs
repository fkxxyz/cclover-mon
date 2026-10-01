use super::*;

pub(super) fn board_config(product_key: &str) -> Option<BoardConfig> {
    match product_key {
        "rogstrixb850egamingwifi" => Some(BoardConfig {
            family: Family::Amd800,
            sensors: &[
                Sensor::TempCPUPackage,
                Sensor::TempVrm,
                Sensor::TempTSensorAlt,
            ],
        }),
        "rogcrosshairx870edarkhero" => Some(BoardConfig {
            family: Family::Amd800,
            sensors: &[Sensor::TempTSensor],
        }),
        "rogcrosshairx870eherobtf" => Some(BoardConfig {
            family: Family::Amd800,
            sensors: &[
                Sensor::TempCPU,
                Sensor::TempCPUPackage,
                Sensor::TempMB,
                Sensor::TempVrm,
                Sensor::TempTSensor,
                Sensor::FanCPUOpt,
            ],
        }),
        "rogcrosshairx870ehero" => Some(BoardConfig {
            family: Family::Amd800,
            sensors: &[
                Sensor::TempCPU,
                Sensor::TempCPUPackage,
                Sensor::TempMB,
                Sensor::TempVrm,
                Sensor::TempTSensor,
                Sensor::FanCPUOpt,
            ],
        }),
        "tufgamingx870pluswifi" => Some(BoardConfig {
            family: Family::Amd800,
            sensors: &[Sensor::TempVrm, Sensor::FanCPUOpt],
        }),
        "rogstrixx870eegamingwifi" => Some(BoardConfig {
            family: Family::Amd800,
            sensors: &[
                Sensor::TempCPU,
                Sensor::TempCPUPackage,
                Sensor::TempMB,
                Sensor::TempVrm,
                Sensor::FanCPUOpt,
            ],
        }),
        "primex470pro" => Some(BoardConfig {
            family: Family::Amd400,
            sensors: &[
                Sensor::TempChipset,
                Sensor::TempCPU,
                Sensor::TempMB,
                Sensor::TempVrm,
                Sensor::FanCPUOpt,
            ],
        }),
        "primex570pro" => Some(BoardConfig {
            family: Family::Amd500,
            sensors: &[
                Sensor::TempChipset,
                Sensor::TempCPU,
                Sensor::TempMB,
                Sensor::TempVrm,
                Sensor::TempTSensor,
                Sensor::FanChipset,
            ],
        }),
        "proartx570creatorwifi" => Some(BoardConfig {
            family: Family::Amd500,
            sensors: &[
                Sensor::TempChipset,
                Sensor::TempCPU,
                Sensor::TempMB,
                Sensor::TempVrm,
                Sensor::TempTSensor,
                Sensor::FanCPUOpt,
            ],
        }),
        "prowsx570ace" => Some(BoardConfig {
            family: Family::Amd500,
            sensors: &[
                Sensor::TempChipset,
                Sensor::TempCPU,
                Sensor::TempMB,
                Sensor::TempVrm,
                Sensor::FanChipset,
            ],
        }),
        "rogcrosshairviiihero" => Some(BoardConfig {
            family: Family::Amd500,
            sensors: &[
                Sensor::TempChipset,
                Sensor::TempCPU,
                Sensor::TempMB,
                Sensor::TempTSensor,
                Sensor::TempVrm,
                Sensor::TempWaterIn,
                Sensor::TempWaterOut,
                Sensor::FanCPUOpt,
                Sensor::FanChipset,
            ],
        }),
        "rogcrosshairviiiherowifi" => Some(BoardConfig {
            family: Family::Amd500,
            sensors: &[
                Sensor::TempChipset,
                Sensor::TempCPU,
                Sensor::TempMB,
                Sensor::TempTSensor,
                Sensor::TempVrm,
                Sensor::TempWaterIn,
                Sensor::TempWaterOut,
                Sensor::FanCPUOpt,
                Sensor::FanChipset,
            ],
        }),
        "rogcrosshairviiiformula" => Some(BoardConfig {
            family: Family::Amd500,
            sensors: &[
                Sensor::TempChipset,
                Sensor::TempCPU,
                Sensor::TempMB,
                Sensor::TempTSensor,
                Sensor::TempVrm,
                Sensor::TempWaterIn,
                Sensor::TempWaterOut,
                Sensor::FanCPUOpt,
                Sensor::FanChipset,
            ],
        }),
        "rogcrosshairx670eextreme" => Some(BoardConfig {
            family: Family::Amd600,
            sensors: &[Sensor::TempWaterIn, Sensor::TempWaterOut, Sensor::FanCPUOpt],
        }),
        "rogcrosshairx670ehero" => Some(BoardConfig {
            family: Family::Amd600,
            sensors: &[Sensor::TempWaterIn, Sensor::TempWaterOut, Sensor::FanCPUOpt],
        }),
        "rogcrosshairx670egene" => Some(BoardConfig {
            family: Family::Amd600,
            sensors: &[Sensor::TempWaterIn, Sensor::TempWaterOut, Sensor::FanCPUOpt],
        }),
        "rogstrixx670eegamingwifi" => Some(BoardConfig {
            family: Family::Amd600,
            sensors: &[Sensor::TempWaterIn, Sensor::TempWaterOut, Sensor::FanCPUOpt],
        }),
        "rogstrixx670efgamingwifi" => Some(BoardConfig {
            family: Family::Amd600,
            sensors: &[Sensor::TempWaterIn, Sensor::TempWaterOut, Sensor::FanCPUOpt],
        }),
        "rogcrosshairviiidarkhero" => Some(BoardConfig {
            family: Family::Amd500,
            sensors: &[
                Sensor::TempChipset,
                Sensor::TempCPU,
                Sensor::TempMB,
                Sensor::TempTSensor,
                Sensor::TempVrm,
                Sensor::TempWaterIn,
                Sensor::TempWaterOut,
                Sensor::FanCPUOpt,
            ],
        }),
        "rogcrosshairviiiimpact" => Some(BoardConfig {
            family: Family::Amd500,
            sensors: &[
                Sensor::TempChipset,
                Sensor::TempCPU,
                Sensor::TempMB,
                Sensor::TempTSensor,
                Sensor::TempVrm,
                Sensor::FanChipset,
            ],
        }),
        "rogstrixb550egaming" => Some(BoardConfig {
            family: Family::Amd500,
            sensors: &[
                Sensor::TempChipset,
                Sensor::TempCPU,
                Sensor::TempMB,
                Sensor::TempTSensor,
                Sensor::TempVrm,
                Sensor::FanCPUOpt,
            ],
        }),
        "rogstrixb550igaming" => Some(BoardConfig {
            family: Family::Amd500,
            sensors: &[
                Sensor::TempChipset,
                Sensor::TempCPU,
                Sensor::TempMB,
                Sensor::TempTSensor,
                Sensor::TempVrm,
                Sensor::FanVrmHS,
            ],
        }),
        "rogstrixx570egaming" => Some(BoardConfig {
            family: Family::Amd500,
            sensors: &[
                Sensor::TempChipset,
                Sensor::TempCPU,
                Sensor::TempMB,
                Sensor::TempTSensor,
                Sensor::TempVrm,
                Sensor::FanChipset,
            ],
        }),
        "rogstrixx570egamingwifiii" => Some(BoardConfig {
            family: Family::Amd500,
            sensors: &[
                Sensor::TempChipset,
                Sensor::TempCPU,
                Sensor::TempMB,
                Sensor::TempTSensor,
                Sensor::TempVrm,
            ],
        }),
        "rogstrixx570fgaming" => Some(BoardConfig {
            family: Family::Amd500,
            sensors: &[
                Sensor::TempChipset,
                Sensor::TempCPU,
                Sensor::TempMB,
                Sensor::TempTSensor,
                Sensor::FanChipset,
            ],
        }),
        "rogstrixx570igaming" => Some(BoardConfig {
            family: Family::Amd500,
            sensors: &[
                Sensor::TempTSensor,
                Sensor::FanVrmHS,
                Sensor::FanChipset,
                Sensor::TempChipset,
                Sensor::TempVrm,
            ],
        }),
        "rogstrixz370ggaming" => Some(BoardConfig {
            family: Family::Intel370,
            sensors: &[
                Sensor::TempChipset,
                Sensor::TempTSensor,
                Sensor::FanCPUOpt,
                Sensor::FanWaterPump,
            ],
        }),
        "rogstrixz390egaming" => Some(BoardConfig {
            family: Family::Intel300,
            sensors: &[
                Sensor::TempVrm,
                Sensor::TempChipset,
                Sensor::TempTSensor,
                Sensor::FanCPUOpt,
            ],
        }),
        "rogstrixz390fgaming" => Some(BoardConfig {
            family: Family::Intel300,
            sensors: &[
                Sensor::TempVrm,
                Sensor::TempChipset,
                Sensor::TempTSensor,
                Sensor::FanCPUOpt,
            ],
        }),
        "rogstrixz390igaming" => Some(BoardConfig {
            family: Family::Intel300,
            sensors: &[Sensor::TempVrm, Sensor::TempChipset, Sensor::TempTSensor],
        }),
        "rogmaximusxiformula" => Some(BoardConfig {
            family: Family::Intel300,
            sensors: &[
                Sensor::TempVrm,
                Sensor::TempChipset,
                Sensor::TempWaterIn,
                Sensor::TempWaterOut,
                Sensor::TempTSensor,
                Sensor::FanCPUOpt,
            ],
        }),
        "rogmaximusxiiformula" => Some(BoardConfig {
            family: Family::Intel400,
            sensors: &[
                Sensor::TempTSensor,
                Sensor::TempVrm,
                Sensor::TempWaterIn,
                Sensor::TempWaterOut,
            ],
        }),
        "rogstrixz690agamingwifid4" => Some(BoardConfig {
            family: Family::Intel600,
            sensors: &[Sensor::TempTSensor, Sensor::TempVrm],
        }),
        "rogstrixz690ggamingwifi" => Some(BoardConfig {
            family: Family::Intel600,
            sensors: &[Sensor::TempTSensor, Sensor::TempVrm],
        }),
        "rogmaximusz690hero" => Some(BoardConfig {
            family: Family::Intel600,
            sensors: &[
                Sensor::TempTSensor,
                Sensor::TempWaterIn,
                Sensor::TempWaterOut,
            ],
        }),
        "rogmaximusz690formula" => Some(BoardConfig {
            family: Family::Intel600,
            sensors: &[
                Sensor::TempTSensor,
                Sensor::TempVrm,
                Sensor::TempWaterIn,
                Sensor::TempWaterOut,
                Sensor::TempWaterBlockIn,
            ],
        }),
        "rogmaximusz690extremeglacial" => Some(BoardConfig {
            family: Family::Intel600,
            sensors: &[
                Sensor::TempVrm,
                Sensor::TempWaterIn,
                Sensor::TempWaterOut,
                Sensor::TempWaterBlockIn,
            ],
        }),
        "rogmaximusz790hero" => Some(BoardConfig {
            family: Family::Intel700,
            sensors: &[
                Sensor::TempVrm,
                Sensor::TempTSensor,
                Sensor::TempWaterIn,
                Sensor::TempWaterOut,
                Sensor::FanCPUOpt,
            ],
        }),
        "rogmaximusz790darkhero" => Some(BoardConfig {
            family: Family::Intel700,
            sensors: &[
                Sensor::TempVrm,
                Sensor::FanCPUOpt,
                Sensor::TempTSensor,
                Sensor::TempWaterIn,
                Sensor::TempWaterOut,
            ],
        }),
        "z170a" => Some(BoardConfig {
            family: Family::Intel100,
            sensors: &[
                Sensor::TempTSensor,
                Sensor::TempChipset,
                Sensor::FanWaterPump,
            ],
        }),
        "z170progaming" => Some(BoardConfig {
            family: Family::Intel100,
            sensors: &[Sensor::TempChipset, Sensor::TempVrm, Sensor::TempTSensor],
        }),
        "primez690a" => Some(BoardConfig {
            family: Family::Intel600,
            sensors: &[Sensor::TempTSensor, Sensor::TempVrm],
        }),
        "rogstrixz790igamingwifi" => Some(BoardConfig {
            family: Family::Intel700,
            sensors: &[Sensor::TempTSensor, Sensor::TempTSensor2],
        }),
        "rogstrixz790egamingwifi" => Some(BoardConfig {
            family: Family::Intel700,
            sensors: &[Sensor::TempWaterIn],
        }),
        "rogstrixz790egamingwifiii" => Some(BoardConfig {
            family: Family::Intel700,
            sensors: &[Sensor::TempTSensor, Sensor::TempVrm, Sensor::FanCPUOpt],
        }),
        "rogstrixz890egamingwifi" => Some(BoardConfig {
            family: Family::Intel800,
            sensors: &[Sensor::TempTSensor, Sensor::TempVrm],
        }),
        "rogmaximusz790formula" => Some(BoardConfig {
            family: Family::Intel700,
            sensors: &[Sensor::TempWaterIn, Sensor::TempWaterOut],
        }),
        "rogmaximusxiiherowifi" => Some(BoardConfig {
            family: Family::Intel400,
            sensors: &[
                Sensor::TempTSensor,
                Sensor::TempChipset,
                Sensor::TempVrm,
                Sensor::TempWaterIn,
                Sensor::TempWaterOut,
                Sensor::FanCPUOpt,
            ],
        }),
        "rogstrixx870igamingwifi" => Some(BoardConfig {
            family: Family::Amd800,
            sensors: &[
                Sensor::TempCPU,
                Sensor::TempCPUPackage,
                Sensor::TempMB,
                Sensor::TempVrm,
            ],
        }),
        _ => None,
    }
}
