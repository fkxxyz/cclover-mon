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
    use crossbeam_channel::unbounded;

    use super::*;

    #[test]
    fn state_bridge_drop_stops_and_joins_worker() {
        let (sender, receiver) = bounded(1);
        let bridge = NativeStateBridge::spawn(receiver, || {});

        drop(bridge);

        assert!(sender.try_send(MonitorState::default()).is_err());
    }

    #[test]
    fn shutdown_waits_for_an_in_flight_wake_before_returning() {
        let (sender, receiver) = bounded(1);
        let (wake_entered_sender, wake_entered_receiver) = bounded(1);
        let (wake_release_sender, wake_release_receiver) = bounded(1);
        let bridge = NativeStateBridge::spawn(receiver, move || {
            wake_entered_sender
                .send(())
                .expect("wake observer must remain connected");
            wake_release_receiver
                .recv()
                .expect("wake release must remain connected");
        });

        sender
            .send(MonitorState::default())
            .expect("bridge receiver must remain connected");
        wake_entered_receiver
            .recv()
            .expect("worker must enter wake callback");

        let (drop_done_sender, drop_done_receiver) = unbounded();
        let drop_thread = std::thread::spawn(move || {
            drop(bridge);
            drop_done_sender
                .send(())
                .expect("drop observer must remain connected");
        });

        assert!(
            drop_done_receiver.try_recv().is_err(),
            "bridge shutdown must join an in-flight wake callback"
        );
        wake_release_sender
            .send(())
            .expect("wake callback must still be waiting");
        drop_done_receiver
            .recv()
            .expect("shutdown must finish after wake callback returns");
        drop_thread.join().expect("bridge drop thread must join");
        assert!(sender.try_send(MonitorState::default()).is_err());
    }
}
