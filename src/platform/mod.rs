use crate::core::model::RawSnapshot;

pub trait Collector: Send + 'static {
    fn collect(&mut self) -> RawSnapshot;
}

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::Backend;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::Backend;

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
compile_error!("cclover-mon currently supports Linux and Windows targets only");
