#![deny(unsafe_code)]

mod app;
mod host;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

pub use app::DesktopApp;
pub use host::run;
