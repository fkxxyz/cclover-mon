#![deny(unsafe_code)]

#[cfg(target_os = "linux")]
mod linux;
#[cfg(any(target_os = "linux", target_os = "windows"))]
mod native;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::run;
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub use native::DesktopApp;
#[cfg(target_os = "windows")]
pub use windows::run;
