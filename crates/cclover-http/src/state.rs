use std::sync::{Arc, Mutex};

use cclover_core::model::MonitorState;
use cclover_presentation::Dashboard;
use crossbeam_channel::{Receiver, Sender, TrySendError, bounded};

use crate::api::ApiV1State;

#[derive(Clone)]
pub(crate) struct StateHub {
    inner: Arc<Mutex<HubState>>,
}

struct HubState {
    latest_dashboard_json: Option<Arc<str>>,
    latest_api_v1: Option<Arc<ApiV1State>>,
    latest_api_v1_json: Option<Arc<str>>,
    subscribers: Vec<Sender<Arc<str>>>,
}

impl StateHub {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HubState {
                latest_dashboard_json: None,
                latest_api_v1: None,
                latest_api_v1_json: None,
                subscribers: Vec::new(),
            })),
        }
    }

    pub(crate) fn publish(&self, state: &MonitorState) {
        let dashboard_html = cclover_web_ui::render(Dashboard::new(state));
        let Ok(dashboard_json) = serde_json::to_string(&dashboard_html) else {
            eprintln!("cclover-mon: failed to serialize browser dashboard");
            return;
        };
        let latest_api_v1 = Arc::new(ApiV1State::from(state));
        let Ok(api_v1_json) = serde_json::to_string(latest_api_v1.as_ref()) else {
            eprintln!("cclover-mon: failed to serialize API v1 state");
            return;
        };
        let dashboard_json: Arc<str> = dashboard_json.into();
        let api_v1_json: Arc<str> = api_v1_json.into();
        let mut inner = self.inner.lock().expect("HTTP state hub lock poisoned");
        inner.latest_dashboard_json = Some(Arc::clone(&dashboard_json));
        inner.latest_api_v1 = Some(latest_api_v1);
        inner.latest_api_v1_json = Some(api_v1_json);
        inner.subscribers.retain(|subscriber| {
            match subscriber.try_send(Arc::clone(&dashboard_json)) {
                Ok(()) | Err(TrySendError::Full(_)) => true,
                Err(TrySendError::Disconnected(_)) => false,
            }
        });
    }

    pub(crate) fn latest_api_v1(&self) -> Option<Arc<ApiV1State>> {
        let inner = self.inner.lock().expect("HTTP state hub lock poisoned");
        inner.latest_api_v1.as_ref().map(Arc::clone)
    }

    pub(crate) fn latest_api_v1_json(&self) -> Option<Arc<str>> {
        let inner = self.inner.lock().expect("HTTP state hub lock poisoned");
        inner.latest_api_v1_json.as_ref().map(Arc::clone)
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.inner
            .lock()
            .expect("HTTP state hub lock poisoned")
            .latest_api_v1
            .is_some()
    }

    pub(crate) fn subscribe(&self) -> (Option<Arc<str>>, Receiver<Arc<str>>) {
        let (sender, receiver) = bounded(1);
        let mut inner = self.inner.lock().expect("HTTP state hub lock poisoned");
        let latest = inner.latest_dashboard_json.as_ref().map(Arc::clone);
        inner.subscribers.push(sender);
        (latest, receiver)
    }
}
