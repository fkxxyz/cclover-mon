#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use cclover_core::model::MonitorState;
use cclover_presentation::Dashboard;
use crossbeam_channel::{Receiver, Sender, TrySendError, bounded};

use crate::api::ApiV1State;

#[derive(Clone)]
pub(crate) struct StateHub {
    inner: Arc<Mutex<HubState>>,
    #[cfg(test)]
    dashboard_renders: Arc<AtomicUsize>,
}

struct HubState {
    latest_state: Option<Arc<MonitorState>>,
    latest_dashboard_json: Option<Arc<str>>,
    latest_api_v1: Option<Arc<ApiV1State>>,
    latest_api_v1_json: Option<Arc<str>>,
    subscribers: Vec<Sender<Arc<str>>>,
}

impl StateHub {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HubState {
                latest_state: None,
                latest_dashboard_json: None,
                latest_api_v1: None,
                latest_api_v1_json: None,
                subscribers: Vec::new(),
            })),
            #[cfg(test)]
            dashboard_renders: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub(crate) fn publish(&self, state: MonitorState) {
        let state = Arc::new(state);
        let latest_api_v1 = Arc::new(ApiV1State::from(state.as_ref()));
        let Ok(api_v1_json) = serde_json::to_string(latest_api_v1.as_ref()) else {
            eprintln!("cclover-mon: failed to serialize API v1 state");
            return;
        };
        let api_v1_json: Arc<str> = api_v1_json.into();
        let should_render = {
            let mut inner = self.inner.lock().expect("HTTP state hub lock poisoned");
            inner.latest_state = Some(Arc::clone(&state));
            inner.latest_dashboard_json = None;
            inner.latest_api_v1 = Some(latest_api_v1);
            inner.latest_api_v1_json = Some(api_v1_json);
            !inner.subscribers.is_empty()
        };

        if !should_render {
            return;
        }

        let Some(dashboard_json) = self.render_dashboard_json(state.as_ref()) else {
            return;
        };
        let mut inner = self.inner.lock().expect("HTTP state hub lock poisoned");
        if !inner
            .latest_state
            .as_ref()
            .is_some_and(|latest| Arc::ptr_eq(latest, &state))
        {
            return;
        }
        inner.latest_dashboard_json = Some(Arc::clone(&dashboard_json));
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
        let state_to_render = if inner.latest_dashboard_json.is_none() {
            inner.latest_state.as_ref().map(Arc::clone)
        } else {
            None
        };
        if let Some(state) = state_to_render {
            inner.latest_dashboard_json = self.render_dashboard_json(state.as_ref());
        }
        let latest = inner.latest_dashboard_json.as_ref().map(Arc::clone);
        inner.subscribers.push(sender);
        (latest, receiver)
    }

    #[cfg(test)]
    pub(crate) fn dashboard_render_count(&self) -> usize {
        self.dashboard_renders.load(Ordering::Relaxed)
    }

    fn render_dashboard_json(&self, state: &MonitorState) -> Option<Arc<str>> {
        #[cfg(test)]
        self.dashboard_renders.fetch_add(1, Ordering::Relaxed);
        render_dashboard_json(state)
    }
}

fn render_dashboard_json(state: &MonitorState) -> Option<Arc<str>> {
    let scene = cclover_ui::build_scene(Dashboard::new(state));
    let dashboard_html = cclover_web_ui::render(&scene);
    match serde_json::to_string(&dashboard_html) {
        Ok(dashboard_json) => Some(dashboard_json.into()),
        Err(error) => {
            eprintln!("cclover-mon: failed to serialize browser dashboard: {error}");
            None
        }
    }
}
