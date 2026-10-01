use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, TrySendError, bounded};

#[derive(Clone, Default)]
pub struct Shutdown {
    inner: Arc<ShutdownInner>,
}

#[derive(Default)]
struct ShutdownInner {
    state: Mutex<ShutdownState>,
    wake: Condvar,
}

#[derive(Default)]
struct ShutdownState {
    requested: bool,
    subscribers: Vec<Sender<()>>,
}

impl Shutdown {
    pub fn request(&self) {
        let mut state = self.inner.state.lock().expect("shutdown lock poisoned");
        if state.requested {
            return;
        }
        state.requested = true;
        state
            .subscribers
            .retain(|subscriber| match subscriber.try_send(()) {
                Ok(()) | Err(TrySendError::Full(_)) => true,
                Err(TrySendError::Disconnected(_)) => false,
            });
        self.inner.wake.notify_all();
    }

    pub fn is_requested(&self) -> bool {
        self.inner
            .state
            .lock()
            .expect("shutdown lock poisoned")
            .requested
    }

    pub fn subscribe(&self) -> Receiver<()> {
        let (sender, receiver) = bounded(1);
        let mut state = self.inner.state.lock().expect("shutdown lock poisoned");
        if state.requested {
            let _ = sender.try_send(());
        } else {
            state.subscribers.push(sender);
        }
        receiver
    }

    pub fn wait(&self) {
        let mut state = self.inner.state.lock().expect("shutdown lock poisoned");
        while !state.requested {
            state = self.inner.wake.wait(state).expect("shutdown lock poisoned");
        }
    }

    pub fn wait_timeout(&self, duration: Duration) -> bool {
        let state = self.inner.state.lock().expect("shutdown lock poisoned");
        if state.requested {
            return true;
        }
        let (state, _) = self
            .inner
            .wake
            .wait_timeout_while(state, duration, |state| !state.requested)
            .expect("shutdown lock poisoned");
        state.requested
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_wakes_waiters_and_is_sticky() {
        let shutdown = Shutdown::default();
        let receiver = shutdown.subscribe();
        let waiter = shutdown.clone();
        let thread = std::thread::spawn(move || waiter.wait_timeout(Duration::from_secs(5)));

        shutdown.request();

        assert!(thread.join().unwrap());
        assert!(shutdown.is_requested());
        assert!(shutdown.wait_timeout(Duration::ZERO));
        assert!(receiver.try_recv().is_ok());
        assert!(shutdown.subscribe().try_recv().is_ok());
    }
}
