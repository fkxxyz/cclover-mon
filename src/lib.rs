#![deny(unsafe_code)]

pub mod core;
pub mod presentation;
pub mod ui;
pub mod web_transport;

#[cfg(any(target_os = "linux", target_os = "windows"))]
pub mod cli;
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub mod platform;
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub mod runtime;
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub mod tui;
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub mod web;

#[cfg(target_arch = "wasm32")]
pub mod web_client;
