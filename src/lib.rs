#![deny(unsafe_code)]

pub mod core;
pub mod presentation;
pub mod ui;

#[cfg(any(target_os = "linux", target_os = "windows"))]
pub mod app;
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub mod cli;
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub mod platform;
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub mod web;

#[cfg(target_arch = "wasm32")]
pub mod web_client;
