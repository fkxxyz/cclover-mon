// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// https://mozilla.org/MPL/2.0/.
// Copyright (C) LibreHardwareMonitor and Contributors.
// ACPI EC board mappings and transaction protocol follow LibreHardwareMonitor
// EmbeddedController.cs / WindowsEmbeddedControllerIO.cs at reviewed commit
// 677a3a56abde9adff5abdb42db3a7638c9137572.

use std::io;
use std::thread;
use std::time::Duration;

use cclover_core::model::{FanId, FanSnapshot, TemperatureId, TemperatureSnapshot};

use super::super::pawnio::Session;
use super::board;
use super::sync::lock_named;

const EC_MUTEX: &str = "Global\\Access_EC";
const EC_WAIT_MS: u32 = 10;
const COMMAND_PORT: u8 = 0x66;
const DATA_PORT: u8 = 0x62;
const READ_COMMAND: u8 = 0x80;
const WRITE_COMMAND: u8 = 0x81;
const OUTPUT_BUFFER_FULL: u8 = 0x01;
const INPUT_BUFFER_FULL: u8 = 0x02;
const MAX_RETRIES: usize = 5;
const WAIT_SPINS: usize = 50;
const FAILURES_BEFORE_SKIP: usize = 20;

#[derive(Clone, Copy)]
pub(super) struct Projection {
    pub temperatures: bool,
    pub fans: bool,
}

pub(super) struct Observation {
    pub temperatures: Vec<TemperatureSnapshot>,
    pub fans: Vec<FanSnapshot>,
    pub temperature_degraded: bool,
    pub fan_degraded: bool,
}

pub(super) struct Collector {
    board_key: String,
    config: BoardConfig,
    wait_read_failures: usize,
}

#[derive(Clone, Copy)]
struct BoardConfig {
    family: Family,
    sensors: &'static [Sensor],
}

#[derive(Clone, Copy)]
enum Family {
    Amd400,
    Amd500,
    Amd600,
    Amd800,
    Intel100,
    Intel300,
    Intel370,
    Intel400,
    Intel600,
    Intel700,
    Intel800,
}

#[allow(clippy::enum_variant_names)]
#[derive(Clone, Copy)]
enum Sensor {
    TempChipset,
    TempCPU,
    TempCPUPackage,
    TempMB,
    TempTSensor,
    TempTSensorAlt,
    TempTSensor2,
    TempVrm,
    TempWaterIn,
    TempWaterOut,
    TempWaterBlockIn,
    FanCPUOpt,
    FanVrmHS,
    FanChipset,
    FanWaterPump,
}

#[derive(Clone, Copy)]
enum Kind {
    Temperature,
    Fan,
}

#[derive(Clone, Copy)]
struct SensorSpec {
    key: &'static str,
    name: &'static str,
    kind: Kind,
    register: u16,
    size: u8,
    blank: Option<i16>,
}

mod boards;
mod collector;
mod sensors;

use boards::board_config;
use sensors::sensor_spec;
