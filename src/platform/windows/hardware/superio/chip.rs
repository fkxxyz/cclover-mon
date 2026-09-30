// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// https://mozilla.org/MPL/2.0/.
// Copyright (C) LibreHardwareMonitor and Contributors.
// Partial Copyright (C) Michael Möller <mmoeller@openhardwaremonitor.org> and Contributors.
// Ported for cclover-mon from LibreHardwareMonitor Chip.cs at reviewed commit
// 677a3a56abde9adff5abdb42db3a7638c9137572.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Chip {
    F71808E,
    F71858,
    F71862,
    F71869,
    F71869A,
    F71878AD,
    F71882,
    F71889AD,
    F71889ED,
    F71889F,
    It8613E,
    It8620E,
    It8625E,
    It8628E,
    It8631E,
    It8638E,
    It8655E,
    It8665E,
    It8686E,
    It8688E,
    It8689E,
    It8696E,
    It8705F,
    It8712F,
    It8716F,
    It8718F,
    It8720F,
    It8721F,
    It8726F,
    It8728F,
    It8771E,
    It8772E,
    It8790E,
    It8792E,
    It87952E,
    Nct610Xd,
    Nct6771F,
    Nct6776F,
    Nct6779D,
    Nct6791D,
    Nct6792D,
    Nct6792Da,
    Nct6793D,
    Nct6795D,
    Nct6796D,
    Nct6796Dr,
    Nct6796Ds,
    Nct6797D,
    Nct6798D,
    Nct6799D,
    Nct5585D,
    Nct6683D,
    Nct6686D,
    Nct6687D,
    Nct6687Dr,
    Nct6701D,
    W83627Dhg,
    W83627Dhgp,
    W83627Ehf,
    W83627Hf,
    W83627Thf,
    W83667Hg,
    W83667Hgb,
    W83687Thf,
}

impl Chip {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::F71808E => "Fintek F71808E",
            Self::F71858 => "Fintek F71858",
            Self::F71862 => "Fintek F71862",
            Self::F71869 => "Fintek F71869",
            Self::F71869A => "Fintek F71869A/F71811",
            Self::F71878AD => "Fintek F71878AD",
            Self::F71882 => "Fintek F71882",
            Self::F71889AD => "Fintek F71889AD",
            Self::F71889ED => "Fintek F71889ED",
            Self::F71889F => "Fintek F71889F",
            Self::It8613E => "ITE IT8613E",
            Self::It8620E => "ITE IT8620E",
            Self::It8625E => "ITE IT8625E",
            Self::It8628E => "ITE IT8628E",
            Self::It8631E => "ITE IT8631E",
            Self::It8638E => "ITE IT8638E",
            Self::It8655E => "ITE IT8655E",
            Self::It8665E => "ITE IT8665E",
            Self::It8686E => "ITE IT8686E",
            Self::It8688E => "ITE IT8688E",
            Self::It8689E => "ITE IT8689E",
            Self::It8696E => "ITE IT8696E",
            Self::It8705F => "ITE IT8705F",
            Self::It8712F => "ITE IT8712F",
            Self::It8716F => "ITE IT8716F",
            Self::It8718F => "ITE IT8718F",
            Self::It8720F => "ITE IT8720F",
            Self::It8721F => "ITE IT8721F",
            Self::It8726F => "ITE IT8726F",
            Self::It8728F => "ITE IT8728F",
            Self::It8771E => "ITE IT8771E",
            Self::It8772E => "ITE IT8772E",
            Self::It8790E => "ITE IT8790E",
            Self::It8792E => "ITE IT8791E/IT8792E/IT8795E",
            Self::It87952E => "ITE IT87952E",
            Self::Nct610Xd => "Nuvoton NCT6102D/NCT6104D/NCT6106D",
            Self::Nct6771F => "Nuvoton NCT6771F",
            Self::Nct6776F => "Nuvoton NCT6776F",
            Self::Nct6779D => "Nuvoton NCT6779D",
            Self::Nct6791D => "Nuvoton NCT6791D",
            Self::Nct6792D => "Nuvoton NCT6792D",
            Self::Nct6792Da => "Nuvoton NCT6792D-A",
            Self::Nct6793D => "Nuvoton NCT6793D",
            Self::Nct6795D => "Nuvoton NCT6795D",
            Self::Nct6796D => "Nuvoton NCT6796D",
            Self::Nct6796Dr => "Nuvoton NCT6796DR",
            Self::Nct6796Ds => "Nuvoton NCT6796DS",
            Self::Nct6797D => "Nuvoton NCT6797D",
            Self::Nct6798D => "Nuvoton NCT6798D",
            Self::Nct6799D => "Nuvoton NCT6799D",
            Self::Nct5585D => "Nuvoton NCT5585D",
            Self::Nct6683D => "Nuvoton NCT6683D",
            Self::Nct6686D => "Nuvoton NCT6686D",
            Self::Nct6687D => "Nuvoton NCT6687D",
            Self::Nct6687Dr => "Nuvoton NCT6687DR",
            Self::Nct6701D => "Nuvoton NCT6701D",
            Self::W83627Dhg => "Winbond W83627DHG",
            Self::W83627Dhgp => "Winbond W83627DHG-P",
            Self::W83627Ehf => "Winbond W83627EHF",
            Self::W83627Hf => "Winbond W83627HF",
            Self::W83627Thf => "Winbond W83627THF",
            Self::W83667Hg => "Winbond W83667HG",
            Self::W83667Hgb => "Winbond W83667HG-B",
            Self::W83687Thf => "Winbond W83687THF",
        }
    }

    pub(super) fn stable_key(self) -> &'static str {
        match self {
            Self::F71808E => "f71808e",
            Self::F71858 => "f71858",
            Self::F71862 => "f71862",
            Self::F71869 => "f71869",
            Self::F71869A => "f71869a",
            Self::F71878AD => "f71878ad",
            Self::F71882 => "f71882",
            Self::F71889AD => "f71889ad",
            Self::F71889ED => "f71889ed",
            Self::F71889F => "f71889f",
            Self::It8613E => "it8613e",
            Self::It8620E => "it8620e",
            Self::It8625E => "it8625e",
            Self::It8628E => "it8628e",
            Self::It8631E => "it8631e",
            Self::It8638E => "it8638e",
            Self::It8655E => "it8655e",
            Self::It8665E => "it8665e",
            Self::It8686E => "it8686e",
            Self::It8688E => "it8688e",
            Self::It8689E => "it8689e",
            Self::It8696E => "it8696e",
            Self::It8705F => "it8705f",
            Self::It8712F => "it8712f",
            Self::It8716F => "it8716f",
            Self::It8718F => "it8718f",
            Self::It8720F => "it8720f",
            Self::It8721F => "it8721f",
            Self::It8726F => "it8726f",
            Self::It8728F => "it8728f",
            Self::It8771E => "it8771e",
            Self::It8772E => "it8772e",
            Self::It8790E => "it8790e",
            Self::It8792E => "it8792e",
            Self::It87952E => "it87952e",
            Self::Nct610Xd => "nct610xd",
            Self::Nct6771F => "nct6771f",
            Self::Nct6776F => "nct6776f",
            Self::Nct6779D => "nct6779d",
            Self::Nct6791D => "nct6791d",
            Self::Nct6792D => "nct6792d",
            Self::Nct6792Da => "nct6792da",
            Self::Nct6793D => "nct6793d",
            Self::Nct6795D => "nct6795d",
            Self::Nct6796D => "nct6796d",
            Self::Nct6796Dr => "nct6796dr",
            Self::Nct6796Ds => "nct6796ds",
            Self::Nct6797D => "nct6797d",
            Self::Nct6798D => "nct6798d",
            Self::Nct6799D => "nct6799d",
            Self::Nct5585D => "nct5585d",
            Self::Nct6683D => "nct6683d",
            Self::Nct6686D => "nct6686d",
            Self::Nct6687D => "nct6687d",
            Self::Nct6687Dr => "nct6687dr",
            Self::Nct6701D => "nct6701d",
            Self::W83627Dhg => "w83627dhg",
            Self::W83627Dhgp => "w83627dhgp",
            Self::W83627Ehf => "w83627ehf",
            Self::W83627Hf => "w83627hf",
            Self::W83627Thf => "w83627thf",
            Self::W83667Hg => "w83667hg",
            Self::W83667Hgb => "w83667hgb",
            Self::W83687Thf => "w83687thf",
        }
    }

    pub(super) fn is_ite(self) -> bool {
        matches!(
            self,
            Self::It8613E
                | Self::It8620E
                | Self::It8625E
                | Self::It8628E
                | Self::It8631E
                | Self::It8638E
                | Self::It8655E
                | Self::It8665E
                | Self::It8686E
                | Self::It8688E
                | Self::It8689E
                | Self::It8696E
                | Self::It8705F
                | Self::It8712F
                | Self::It8716F
                | Self::It8718F
                | Self::It8720F
                | Self::It8721F
                | Self::It8726F
                | Self::It8728F
                | Self::It8771E
                | Self::It8772E
                | Self::It8790E
                | Self::It8792E
                | Self::It87952E
        )
    }

    pub(super) fn is_fintek(self) -> bool {
        matches!(
            self,
            Self::F71808E
                | Self::F71858
                | Self::F71862
                | Self::F71869
                | Self::F71869A
                | Self::F71878AD
                | Self::F71882
                | Self::F71889AD
                | Self::F71889ED
                | Self::F71889F
        )
    }

    pub(super) fn is_winbond(self) -> bool {
        matches!(
            self,
            Self::W83627Dhg
                | Self::W83627Dhgp
                | Self::W83627Ehf
                | Self::W83627Hf
                | Self::W83627Thf
                | Self::W83667Hg
                | Self::W83667Hgb
                | Self::W83687Thf
        )
    }
}
