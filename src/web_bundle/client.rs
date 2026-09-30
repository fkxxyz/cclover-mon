use std::cell::RefCell;

use cclover_web_ui::BrowserRenderer;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen::closure::Closure;
use web_sys::{EventSource, MessageEvent};

use cclover_core::model::MonitorState;
use cclover_presentation::Dashboard;

use crate::transport::WebMonitorState;

struct WebClient {
    _source: EventSource,
    _on_message: Closure<dyn FnMut(MessageEvent)>,
}

thread_local! {
    static CLIENT: RefCell<Option<WebClient>> = const { RefCell::new(None) };
}

pub fn run() -> Result<(), JsValue> {
    let renderer = BrowserRenderer::mount()?;
    let initial = MonitorState::default();
    renderer.render(Dashboard::new(&initial))?;

    let source = EventSource::new("/events")?;
    let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
        let Some(payload) = event.data().as_string() else {
            return;
        };
        let Ok(state) = serde_json::from_str::<WebMonitorState>(&payload) else {
            return;
        };
        let state = MonitorState::from(state);
        if let Err(error) = renderer.render(Dashboard::new(&state)) {
            web_sys::console::error_1(&error);
        }
    });
    source.set_onmessage(Some(on_message.as_ref().unchecked_ref()));

    CLIENT.with(|slot| {
        *slot.borrow_mut() = Some(WebClient {
            _source: source,
            _on_message: on_message,
        });
    });
    Ok(())
}
