use crate::core::model::RawSnapshot;
use crate::platform::Collector;

pub struct Backend;

impl Backend {
    pub fn new() -> Self {
        Self
    }
}

impl Collector for Backend {
    fn collect(&mut self) -> RawSnapshot {
        // Windows collection belongs here and must produce the same platform-neutral
        // contracts as the Linux backend. The legacy Quickshell product being ported
        // had no Windows collector, so this target intentionally starts unavailable
        // rather than leaking Windows API types into core or UI.
        RawSnapshot::default()
    }
}
