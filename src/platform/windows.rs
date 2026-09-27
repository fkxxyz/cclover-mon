use crate::core::Collector;
use crate::core::model::RawSnapshot;
use crate::platform::{ProbeKind, ProbeReport};

pub struct Backend;

impl Backend {
    pub fn new() -> Self {
        Self
    }

    pub fn probe(&mut self, kind: ProbeKind) -> ProbeReport {
        ProbeReport {
            available: false,
            summary: vec![format!("{} collector is unavailable", kind.as_str())],
            raw: Vec::new(),
            notes: vec!["Windows collection is not implemented yet".to_owned()],
        }
    }

    pub fn collect_for_perf(&mut self, _kind: ProbeKind) {
        std::hint::black_box(self.collect());
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
