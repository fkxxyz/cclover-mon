use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use cclover_core::model::MonitorState;
use crossbeam_channel::{Receiver, Sender, bounded, select};

pub struct DesktopApp {
    pub(crate) receiver: Receiver<MonitorState>,
}

impl DesktopApp {
    pub fn new(receiver: Receiver<MonitorState>) -> Self {
        Self { receiver }
    }
}
pub(crate) struct NativeStateBridge {
    pending: Arc<Mutex<Option<MonitorState>>>,
    shutdown: Sender<()>,
    thread: Option<JoinHandle<()>>,
}

impl NativeStateBridge {
    pub(crate) fn spawn<F>(receiver: Receiver<MonitorState>, wake: F) -> Self
    where
        F: Fn() + Send + 'static,
    {
        let pending = Arc::new(Mutex::new(None));
        let bridge_pending = Arc::clone(&pending);
        let (shutdown, shutdown_receiver) = bounded(1);
        let thread = thread::Builder::new()
            .name("cclover-mon-desktop-state".to_owned())
            .spawn(move || {
                loop {
                    select! {
                        recv(shutdown_receiver) -> _ => break,
                        recv(receiver) -> state => match state {
                            Ok(state) => {
                                *bridge_pending
                                    .lock()
                                    .expect("native pending state lock poisoned") = Some(state);
                                wake();
                            }
                            Err(_) => break,
                        },
                    }
                }
            })
            .expect("failed to spawn native desktop state bridge");
        Self {
            pending,
            shutdown,
            thread: Some(thread),
        }
    }

    pub(crate) fn pending(&self) -> Arc<Mutex<Option<MonitorState>>> {
        Arc::clone(&self.pending)
    }
}

impl Drop for NativeStateBridge {
    fn drop(&mut self) {
        let _ = self.shutdown.try_send(());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_bridge_drop_stops_and_joins_worker() {
        let (sender, receiver) = bounded(1);
        let bridge = NativeStateBridge::spawn(receiver, || {});

        drop(bridge);

        assert!(sender.try_send(MonitorState::default()).is_err());
    }
}
