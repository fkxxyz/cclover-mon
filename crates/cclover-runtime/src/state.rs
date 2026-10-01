use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

use cclover_core::model::MonitorState;
use crossbeam_channel::{Receiver, Sender, TrySendError, bounded};

#[derive(Clone)]
pub struct StateSource {
    inner: Arc<Mutex<HubState>>,
}

struct HubState {
    latest: Option<MonitorState>,
    subscribers: Vec<Sender<MonitorState>>,
}

impl Default for StateSource {
    fn default() -> Self {
        Self::new()
    }
}

impl Hash for StateSource {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.inner).hash(state);
    }
}

impl StateSource {
    pub fn new() -> Self {
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

    pub fn latest(&self) -> Option<MonitorState> {
        self.inner
            .lock()
            .expect("native state hub lock poisoned")
            .latest
            .clone()
    }

    pub(crate) fn publish(&self, state: &MonitorState) {
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
