use super::*;

impl Default for Collector {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Source<T> {
    pub(super) fn new() -> Self {
        Self {
            state: SourceState::Uninitialized,
        }
    }

    pub(super) fn ensure_ready(
        &mut self,
        now: Instant,
        initialize: impl FnOnce() -> Result<T, InitFailure>,
    ) -> Result<(), CollectionUnavailable> {
        let should_initialize = match &self.state {
            SourceState::Uninitialized => true,
            SourceState::Retry { at, .. } => now >= *at,
            SourceState::Ready(_) | SourceState::StableUnavailable(_) => false,
        };

        if should_initialize {
            self.state = match initialize() {
                Ok(runtime) => SourceState::Ready(runtime),
                Err(InitFailure::Retry(reason)) => SourceState::Retry {
                    at: now + RETRY_INTERVAL,
                    reason,
                },
                Err(InitFailure::Stable(reason)) => SourceState::StableUnavailable(reason),
            };
        }

        match &self.state {
            SourceState::Ready(_) => Ok(()),
            SourceState::Retry { reason, .. } | SourceState::StableUnavailable(reason) => {
                Err(*reason)
            }
            SourceState::Uninitialized => unreachable!("source initialization handled above"),
        }
    }

    pub(super) fn ready_mut(&mut self) -> Option<&mut T> {
        match &mut self.state {
            SourceState::Ready(runtime) => Some(runtime),
            _ => None,
        }
    }

    pub(super) fn retry(&mut self, now: Instant, reason: CollectionUnavailable) {
        self.state = SourceState::Retry {
            at: now + RETRY_INTERVAL,
            reason,
        };
    }
}
