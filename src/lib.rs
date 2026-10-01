#![deny(unsafe_code)]

pub mod core;
pub mod presentation;

#[cfg(any(target_os = "linux", target_os = "windows"))]
pub mod cli;
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub mod launch;
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub use cclover_platform as platform;
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub mod runtime;
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub mod tui;
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub mod web;
