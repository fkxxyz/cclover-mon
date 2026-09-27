#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod devlog;
#[cfg(not(target_arch = "wasm32"))]
mod history;
pub mod model;
#[cfg(not(target_arch = "wasm32"))]
mod sampler;

#[cfg(not(target_arch = "wasm32"))]
pub use sampler::{Collector, SAMPLE_INTERVAL, SampleCycle, Sampler};
