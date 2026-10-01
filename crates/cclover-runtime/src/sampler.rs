use std::thread::{self, JoinHandle};

use cclover_core::{SampleCycle, Sampler};
use cclover_platform::Backend;

use crate::{Shutdown, StateSource};

pub struct NativeRuntime {
    states: StateSource,
    shutdown: Shutdown,
    sampler_thread: Option<JoinHandle<()>>,
}

impl NativeRuntime {
    pub fn start() -> std::io::Result<Self> {
        Self::start_with_shutdown(Shutdown::default())
    }

    pub fn start_with_shutdown(shutdown: Shutdown) -> std::io::Result<Self> {
        let states = StateSource::new();
        let sampler_states = states.clone();
        let sampler_shutdown = shutdown.clone();
        let sampler_thread = thread::Builder::new()
            .name("cclover-mon-sampler".to_owned())
            .spawn(move || {
                let mut sampler = Sampler::new(Backend::new());
                while !sampler_shutdown.is_requested() {
                    let cycle = SampleCycle::begin();
                    let state = sampler.sample();
                    sampler_states.publish(&state);
                    if sampler_shutdown.wait_timeout(cycle.remaining()) {
                        break;
                    }
                }
            })?;

        Ok(Self {
            states,
            shutdown,
            sampler_thread: Some(sampler_thread),
        })
    }

    pub fn states(&self) -> StateSource {
        self.states.clone()
    }

    pub fn shutdown(&self) -> Shutdown {
        self.shutdown.clone()
    }
}

impl Drop for NativeRuntime {
    fn drop(&mut self) {
        self.shutdown.request();
        if let Some(thread) = self.sampler_thread.take() {
            let _ = thread.join();
        }
    }
}
