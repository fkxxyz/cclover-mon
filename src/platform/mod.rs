use std::str::FromStr;

use crate::core::model::CollectionStatus;

macro_rules! define_probe_kinds {
    ($($variant:ident => { name: $name:literal, aliases: [$($alias:literal),* $(,)?], follow_up: $follow_up:expr }),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum ProbeKind {
            $($variant),+
        }

        impl ProbeKind {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            pub fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $name),+
                }
            }

            fn aliases(self) -> &'static [&'static str] {
                match self {
                    $(Self::$variant => &[$($alias),*]),+
                }
            }

            pub fn needs_probe_follow_up(self) -> bool {
                match self {
                    $(Self::$variant => $follow_up),+
                }
            }

            pub fn names_csv() -> String {
                Self::ALL
                    .iter()
                    .map(|kind| kind.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        }

        impl FromStr for ProbeKind {
            type Err = String;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::ALL
                    .iter()
                    .copied()
                    .find(|kind| value == kind.as_str() || kind.aliases().contains(&value))
                    .ok_or_else(|| {
                        format!(
                            "unknown collector {value:?}; expected {}",
                            Self::names_csv()
                        )
                    })
            }
        }
    };
}

define_probe_kinds! {
    Cpu => { name: "cpu", aliases: [], follow_up: false },
    Memory => { name: "memory", aliases: [], follow_up: false },
    Processes => { name: "processes", aliases: ["process"], follow_up: false },
    Network => { name: "network", aliases: ["networks"], follow_up: false },
    NetworkAttribution => { name: "network-attribution", aliases: [], follow_up: true },
    Disk => { name: "disk", aliases: ["disks"], follow_up: false },
    DiskAttribution => { name: "disk-attribution", aliases: [], follow_up: true },
    Temperatures => { name: "temperatures", aliases: ["temperature"], follow_up: false },
    Gpu => { name: "gpu", aliases: ["gpu-memory", "vram"], follow_up: false },
}

pub struct ProbeReport {
    pub status: CollectionStatus,
    pub summary: Vec<String>,
    pub raw: Vec<String>,
    pub notes: Vec<String>,
}

mod probe;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::Backend;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::Backend;
#[cfg(target_os = "windows")]
pub use windows::early_command_exit_code;
#[cfg(target_os = "windows")]
pub use windows::prepare_machine_capability;

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
compile_error!("cclover-mon currently supports Linux and Windows targets only");

#[cfg(test)]
mod tests {
    use super::ProbeKind;

    #[test]
    fn canonical_probe_names_round_trip() {
        for kind in ProbeKind::ALL {
            assert_eq!(kind.as_str().parse::<ProbeKind>(), Ok(*kind));
        }
    }

    #[test]
    fn legacy_probe_aliases_still_parse() {
        assert_eq!("process".parse(), Ok(ProbeKind::Processes));
        assert_eq!("networks".parse(), Ok(ProbeKind::Network));
        assert_eq!("disks".parse(), Ok(ProbeKind::Disk));
        assert_eq!("temperature".parse(), Ok(ProbeKind::Temperatures));
    }
}
