use super::super::*;
use super::tc;

pub(super) fn nct6701_channels() -> Vec<TemperatureChannel> {
    vec![
        tc("peci0", "PECI #0", Some(16), 0x073, 0, -1, 0x100, None),
        tc("cpu", "CPU", Some(2), 0x491, 0, -1, 0, None),
        tc("system", "System", Some(1), 0x490, 0, -1, 0, None),
        tc("aux0", "Auxiliary #1", Some(3), 0x492, 0, -1, 0, None),
        tc("aux1", "Auxiliary #2", Some(4), 0x493, 0, -1, 0, None),
        tc("aux2", "Auxiliary #3", Some(5), 0x494, 0, -1, 0, None),
        tc("aux3", "Auxiliary #4", Some(6), 0x495, 0, -1, 0, None),
        tc("aux4", "Auxiliary #5", Some(7), 0x027, 0, -1, 0x621, None),
        tc("peci1", "PECI #1", Some(17), 0x672, 0, -1, 0xC27, None),
        tc(
            "pch-cpu-max",
            "PCH CPU Max",
            Some(18),
            0x674,
            0,
            -1,
            0xC28,
            Some(0x400),
        ),
        tc(
            "pch-chip",
            "PCH Chip",
            Some(19),
            0x676,
            0,
            -1,
            0xC29,
            Some(0x401),
        ),
        tc(
            "pch-cpu",
            "PCH CPU",
            Some(20),
            0x678,
            0,
            -1,
            0xC2A,
            Some(0x402),
        ),
        tc(
            "pch-mch",
            "PCH MCH",
            Some(21),
            0x67A,
            0,
            -1,
            0xC2B,
            Some(0x404),
        ),
        tc("dimm-a0", "DIMM A0", Some(22), 0x405, 0, -1, 0, None),
        tc("dimm-a1", "DIMM A1", Some(23), 0x406, 0, -1, 0, None),
        tc("dimm-b0", "DIMM B0", Some(24), 0x407, 0, -1, 0, None),
        tc("dimm-b1", "DIMM B1", Some(25), 0x408, 0, -1, 0, None),
        tc("smbus0", "SMBus #0", Some(8), 0x150, 0, -1, 0x622, None),
        tc("smbus1", "SMBus #1", Some(9), 0x670, 0, -1, 0xC26, None),
        tc(
            "byte0",
            "Byte Temperature #1",
            Some(26),
            0x419,
            0,
            -1,
            0,
            None,
        ),
        tc(
            "byte1",
            "Byte Temperature #2",
            Some(27),
            0x41A,
            0,
            -1,
            0,
            None,
        ),
        tc("peci0-cal", "PECI #0 Cal", Some(28), 0x4F4, 0, -1, 0, None),
        tc("peci1-cal", "PECI #1 Cal", Some(29), 0x4F5, 0, -1, 0, None),
        tc("virtual", "Virtual", Some(31), 0, 0, -1, 0, None),
        tc("spare0", "Spare #1", Some(32), 0x07B, 0, -1, 0x900, None),
        tc("spare1", "Spare #2", Some(33), 0, 0, -1, 0, None),
        tc("cpu-package", "CPU Package", None, 0x409, 0, -1, 0, None),
        tc("temp14", "Temperature #14", None, 0x4A2, 0, -1, 0, None),
    ]
}

pub(super) fn nct6796ds_channels() -> Vec<TemperatureChannel> {
    vec![
        tc("cpu", "CPU", Some(2), 0x073, 0x074, 7, 0x100, Some(0x491)),
        tc(
            "system",
            "System",
            Some(1),
            0x075,
            0x076,
            7,
            0x200,
            Some(0x490),
        ),
        tc(
            "aux0",
            "Auxiliary #1",
            Some(3),
            0x077,
            0x078,
            7,
            0x300,
            Some(0x492),
        ),
        tc(
            "aux1",
            "Auxiliary #2",
            Some(4),
            0x079,
            0x07A,
            7,
            0x800,
            Some(0x493),
        ),
        tc(
            "aux2",
            "Auxiliary #3",
            Some(5),
            0x07B,
            0x07C,
            7,
            0x900,
            Some(0x494),
        ),
        tc(
            "aux3",
            "Auxiliary #4",
            Some(6),
            0x07D,
            0x07E,
            7,
            0xA00,
            Some(0x495),
        ),
        tc(
            "aux4",
            "Auxiliary #5",
            Some(7),
            0x027,
            0,
            4,
            0xC16,
            Some(0x496),
        ),
        tc(
            "aux5",
            "Auxiliary #6",
            Some(34),
            0x449,
            0,
            4,
            0x100,
            Some(0x4A2),
        ),
        tc("smbus0", "SMBus #0", Some(8), 0x150, 0x151, 7, 0x622, None),
        tc("peci0", "PECI #0", Some(16), 0x720, 0, -1, 0, None),
        tc("virtual", "Virtual", Some(31), 0, 0, -1, 0, None),
    ]
}

pub(super) fn nct5585_channels() -> Vec<TemperatureChannel> {
    vec![
        tc("peci0", "PECI #0", Some(16), 0x720, 0, -1, 0x100, None),
        tc("cpu", "CPU", Some(2), 0x075, 0x076, 7, 0, Some(0x073)),
        tc(
            "aux1",
            "Auxiliary #2",
            Some(4),
            0x07B,
            0x07C,
            7,
            0x900,
            Some(0x493),
        ),
        tc(
            "aux3",
            "Auxiliary #4",
            Some(6),
            0x4A0,
            0x49E,
            6,
            0xB00,
            Some(0x495),
        ),
    ]
}

pub(super) fn nct6687dr_channels() -> Vec<TemperatureChannel> {
    [
        ("cpu", "CPU"),
        ("system", "System"),
        ("mos", "MOS"),
        ("pch", "PCH"),
        ("cpu-socket", "CPU Socket"),
        ("pcie1", "PCIe #1"),
        ("m2-1", "M2 #1"),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (key, name))| tc(key, name, None, 0x100 + index as u16 * 2, 0, -1, 0, None))
    .collect()
}

pub(super) fn nct668x_channels() -> Vec<TemperatureChannel> {
    let names = [
        ("cpu", "CPU"),
        ("system", "System"),
        ("mos", "MOS"),
        ("pch", "PCH"),
        ("cpu-socket", "CPU Socket"),
        ("pcie1", "PCIe #1"),
        ("m2-1", "M2 #1"),
        ("pcie2", "PCIe #2"),
        ("pcie3", "PCIe #3"),
        ("m2-2", "M2 #2"),
        ("m2-4", "M2 #4"),
    ];
    names
        .into_iter()
        .enumerate()
        .map(|(index, (key, name))| tc(key, name, None, 0x100 + (index as u16 * 2), 0, -1, 0, None))
        .collect()
}
