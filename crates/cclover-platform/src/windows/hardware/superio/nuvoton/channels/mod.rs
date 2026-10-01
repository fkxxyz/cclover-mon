use super::*;
pub(super) fn temperature_channels(chip: Chip) -> Vec<TemperatureChannel> {
    match chip {
        Chip::Nct610Xd => vec![
            tc("peci0", "PECI #0", Some(12), 0x06B, 0, -1, 0x621, None),
            tc("aux", "Auxiliary", Some(2), 0x010, 0x016, 0, 0, None),
            tc("cpu", "CPU", Some(1), 0x011, 0x01B, 1, 0, None),
            tc("system0", "System #1", Some(3), 0x012, 0x01B, 2, 0, None),
            tc("system1", "System #2", Some(4), 0x013, 0x016, 3, 0, None),
            tc("system2", "System #3", Some(5), 0x014, 0x01B, 4, 0, None),
            tc("system3", "System #4", Some(6), 0x015, 0x01B, 5, 0, None),
        ],
        Chip::Nct6771F => legacy_677x_channels(5),
        Chip::Nct6776F => legacy_677x_channels(12),
        Chip::Nct6683D | Chip::Nct6686D | Chip::Nct6687D => nct668x_channels(),
        Chip::Nct6687Dr => nct6687dr_channels(),
        Chip::Nct6701D => nct6701_channels(),
        Chip::Nct6793D | Chip::Nct6795D | Chip::Nct6791D | Chip::Nct6792D | Chip::Nct6792Da => {
            modern_channels(false)
        }
        Chip::Nct6796D | Chip::Nct6796Dr | Chip::Nct6797D => modern_channels(false),
        Chip::Nct6796Ds => nct6796ds_channels(),
        Chip::Nct6798D | Chip::Nct6799D => modern_channels(true),
        Chip::Nct5585D => nct5585_channels(),
        _ => default_67xx_channels(),
    }
}

const fn tc(
    key: &'static str,
    name: &'static str,
    source: Option<u8>,
    register: u16,
    half_register: u16,
    half_bit: i8,
    source_register: u16,
    alternate_register: Option<u16>,
) -> TemperatureChannel {
    TemperatureChannel::new(
        key,
        name,
        source,
        register,
        half_register,
        half_bit,
        source_register,
        alternate_register,
    )
}

mod standard;
mod variants;
use standard::{default_67xx_channels, legacy_677x_channels, modern_channels};
use variants::{
    nct668x_channels, nct5585_channels, nct6687dr_channels, nct6701_channels, nct6796ds_channels,
};
