// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// https://mozilla.org/MPL/2.0/.
// Copyright (C) LibreHardwareMonitor and Contributors.
// ChromeOS EC discovery/decoding follows LibreHardwareMonitor
// ChromeOSEmbeddedController.cs and ChromeOSEmbeddedControllerIO.cs at reviewed commit
// 677a3a56abde9adff5abdb42db3a7638c9137572.

use std::io;

use crate::core::model::{FanId, FanSnapshot, TemperatureId, TemperatureSnapshot};

use super::super::pawnio::Session;
use super::ec::Projection;
use super::sync::lock_named;

const EC_MUTEX: &str = "Global\\Access_EC";
const EC_WAIT_MS: u32 = 100;
const MEMMAP_SIZE: usize = 0x20;
const TEMP_A_START: usize = 0x00;
const TEMP_A_COUNT: usize = 16;
const FAN_START: usize = 0x10;
const FAN_COUNT: usize = 4;
const TEMP_B_START: usize = 0x18;
const TEMP_B_COUNT: usize = 8;
const TEMP_NOT_PRESENT: u8 = 0xFF;
const FAN_NOT_PRESENT: u16 = 0xFFFF;
const TEMP_CELSIUS_OFFSET: i32 = 200 - 273;
const EC_CMD_TEMP_SENSOR_GET_INFO: u64 = 0x0070;

pub(super) struct Collector {
    board_key: String,
    temperatures: Vec<TemperatureChannel>,
    fans: Vec<FanChannel>,
}

pub(super) struct Observation {
    pub temperatures: Vec<TemperatureSnapshot>,
    pub fans: Vec<FanSnapshot>,
    pub temperature_degraded: bool,
    pub fan_degraded: bool,
}

struct TemperatureChannel {
    sensor_index: u8,
    offset: u8,
    name: String,
}

struct FanChannel {
    index: u8,
    offset: u8,
}

impl Collector {
    pub(super) fn discover(session: &Session, board_key: String) -> io::Result<Self> {
        let _guard = lock_named(EC_MUTEX, EC_WAIT_MS, "ChromeOS EC")?;
        let data = read_memmap(session, 0, MEMMAP_SIZE as u8)?;

        let mut temperatures = Vec::new();
        for index in 0..TEMP_A_COUNT {
            let raw = data[TEMP_A_START + index];
            if raw == TEMP_NOT_PRESENT {
                break;
            }
            temperatures.push(TemperatureChannel {
                sensor_index: index as u8,
                offset: (TEMP_A_START + index) as u8,
                name: temperature_name(session, index as u8),
            });
        }
        for index in 0..TEMP_B_COUNT {
            let raw = data[TEMP_B_START + index];
            if raw == TEMP_NOT_PRESENT {
                break;
            }
            let sensor_index = (TEMP_A_COUNT + index) as u8;
            temperatures.push(TemperatureChannel {
                sensor_index,
                offset: (TEMP_B_START + index) as u8,
                name: temperature_name(session, sensor_index),
            });
        }

        let mut fans = Vec::new();
        for index in 0..FAN_COUNT {
            let offset = FAN_START + index * 2;
            let rpm = u16::from_le_bytes([data[offset], data[offset + 1]]);
            if rpm == FAN_NOT_PRESENT {
                break;
            }
            fans.push(FanChannel {
                index: index as u8,
                offset: offset as u8,
            });
        }

        Ok(Self {
            board_key,
            temperatures,
            fans,
        })
    }

    pub(super) fn supports_temperatures(&self) -> bool {
        !self.temperatures.is_empty()
    }

    pub(super) fn supports_fans(&self) -> bool {
        !self.fans.is_empty()
    }

    pub(super) fn collect(
        &self,
        session: &Session,
        projection: Projection,
    ) -> io::Result<Observation> {
        let _guard = lock_named(EC_MUTEX, EC_WAIT_MS, "ChromeOS EC")?;
        let data = read_memmap(session, 0, MEMMAP_SIZE as u8)?;
        let want_temperatures = projection.temperatures;
        let want_fans = projection.fans;
        let mut temperatures = Vec::new();
        let mut fans = Vec::new();
        let mut temperature_degraded = false;
        let mut fan_degraded = false;

        if want_temperatures {
            for channel in &self.temperatures {
                let raw = data[channel.offset as usize];
                if raw == TEMP_NOT_PRESENT {
                    temperature_degraded = true;
                    continue;
                }
                temperatures.push(TemperatureSnapshot {
                    id: TemperatureId::from_opaque_key(format!(
                        "windows:cros-ec:{}:temp:{}",
                        self.board_key, channel.sensor_index
                    )),
                    name: channel.name.clone(),
                    celsius: f64::from(i32::from(raw) + TEMP_CELSIUS_OFFSET),
                });
            }
        }

        if want_fans {
            for channel in &self.fans {
                let offset = channel.offset as usize;
                let rpm = u16::from_le_bytes([data[offset], data[offset + 1]]);
                if rpm == FAN_NOT_PRESENT {
                    fan_degraded = true;
                    continue;
                }
                fans.push(FanSnapshot {
                    id: FanId::from_opaque_key(format!(
                        "windows:cros-ec:{}:fan:{}",
                        self.board_key, channel.index
                    )),
                    name: format!("Fan {}", channel.index + 1),
                    rpm: u64::from(rpm),
                });
            }
        }

        Ok(Observation {
            temperatures,
            fans,
            temperature_degraded,
            fan_degraded,
        })
    }
}

pub(super) fn is_supported_board(product_key: &str) -> bool {
    matches!(
        product_key,
        "franbmcp06"
            | "franbmcp0a"
            | "franbmcp0c"
            | "franmacp04"
            | "franmacp06"
            | "franmacp08"
            | "franmbcp04"
            | "franmccp04"
            | "franmccp06"
            | "franmccp07"
            | "franmdcp05"
            | "franmdcp07"
            | "franmecp02"
            | "franmecp05"
            | "franmecp06"
            | "franmzcp07"
            | "franmzcp09"
            | "franmfcp02"
            | "franmfcp04"
            | "franmfcp06"
            | "frapmacp03"
            | "frapmacp05"
            | "franmgcp05"
            | "franmgcp07"
            | "franmgcp09"
    )
}

fn read_memmap(session: &Session, offset: u8, bytes: u8) -> io::Result<Vec<u8>> {
    let output_words = usize::from(bytes).div_ceil(8);
    let words = session.execute(
        "ioctl_ec_readmem",
        &[u64::from(offset), u64::from(bytes)],
        output_words,
    )?;
    Ok(unpack_words(&words, usize::from(bytes)))
}

fn temperature_name(session: &Session, index: u8) -> String {
    let fallback = || format!("Temp {index}");
    let Ok(words) = session.execute(
        "ioctl_ec_command",
        &[0, EC_CMD_TEMP_SENSOR_GET_INFO, 1, 33, u64::from(index)],
        6,
    ) else {
        return fallback();
    };
    if (words.first().copied().unwrap_or(u64::MAX) as i64) < 0 {
        return fallback();
    }
    let bytes = unpack_words(words.get(1..).unwrap_or_default(), 33);
    let end = bytes
        .iter()
        .take(32)
        .position(|&byte| byte == 0)
        .unwrap_or(32);
    let name = String::from_utf8_lossy(&bytes[..end]).trim().to_owned();
    if name.is_empty() { fallback() } else { name }
}

fn unpack_words(words: &[u64], bytes: usize) -> Vec<u8> {
    words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .take(bytes)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cros_board_allowlist_matches_reviewed_lhm_scope() {
        assert!(is_supported_board("franbmcp06"));
        assert!(is_supported_board("franmgcp09"));
        assert!(!is_supported_board("franbmcp03"));
    }

    #[test]
    fn unpack_words_preserves_little_endian_memmap_bytes() {
        assert_eq!(
            unpack_words(&[0x0807_0605_0403_0201], 5),
            vec![1, 2, 3, 4, 5]
        );
    }
}
