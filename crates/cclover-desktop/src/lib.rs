#![deny(unsafe_code)]

mod app;
mod host;
#[cfg(target_os = "linux")]
mod linux;

pub use app::DesktopApp;
pub use host::run;
