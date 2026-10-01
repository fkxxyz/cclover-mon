use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender, TrySendError, bounded};

use crate::core::model::MonitorState;
use crate::core::{SampleCycle, Sampler};
use crate::platform::Backend;
use crate::web::StateHub;

#[derive(Clone)]
pub struct StateSource {
    inner: Arc<Mutex<HubState>>,
}

struct HubState {
    latest: Option<MonitorState>,
    subscribers: Vec<Sender<MonitorState>>,
}

impl Hash for StateSource {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.inner).hash(state);
    }
}

impl StateSource {
    fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HubState {
                latest: None,
                subscribers: Vec::new(),
            })),
        }
    }

    pub fn subscribe(&self) -> Receiver<MonitorState> {
        let (sender, receiver) = bounded(1);
        let mut inner = self.inner.lock().expect("native state hub lock poisoned");
        if let Some(latest) = &inner.latest {
            let _ = sender.try_send(latest.clone());
        }
        inner.subscribers.push(sender);
        receiver
    }

    fn publish(&self, state: &MonitorState) {
        let mut inner = self.inner.lock().expect("native state hub lock poisoned");
        inner.latest = Some(state.clone());
        inner
            .subscribers
            .retain(|subscriber| match subscriber.try_send(state.clone()) {
                Ok(()) | Err(TrySendError::Full(_)) => true,
                Err(TrySendError::Disconnected(_)) => false,
            });
    }
}

pub struct NativeRuntime {
    states: StateSource,
    shutdown: Sender<()>,
    sampler_thread: Option<JoinHandle<()>>,
}

impl NativeRuntime {
    pub fn start(web_state: Option<StateHub>) -> std::io::Result<Self> {
        let states = StateSource::new();
        let sampler_states = states.clone();
        let (shutdown, shutdown_receiver) = bounded(1);
        let sampler_thread = thread::Builder::new()
            .name("cclover-mon-sampler".to_owned())
            .spawn(move || {
                let mut sampler = Sampler::new(Backend::new());
                loop {
                    let cycle = SampleCycle::begin();
                    let state = sampler.sample();
                    sampler_states.publish(&state);
                    if let Some(web_state) = &web_state {
                        web_state.publish(&state);
                    }
                    let remaining = cycle.remaining();
                    // Sampling cadence supplies the deadline; the shutdown channel can interrupt it immediately.
                    match shutdown_receiver.recv_timeout(remaining) {
                        Ok(()) | Err(RecvTimeoutError::Disconnected) => break,
                        Err(RecvTimeoutError::Timeout) => {}
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
}

impl Drop for NativeRuntime {
    fn drop(&mut self) {
        let _ = self.shutdown.try_send(());
        if let Some(thread) = self.sampler_thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn late_subscriber_receives_latest_state_immediately() {
        let source = StateSource::new();
        let state = MonitorState::default();
        source.publish(&state);

        assert!(source.subscribe().try_recv().is_ok());
    }
}
