use std::time::{Duration, Instant};

use super::devlog;
use super::history;
use super::model::*;

mod cpu;
mod derive;
mod process;
mod rates;
mod status;

use derive::derive;

pub const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);
const HISTORY_CAPACITY: usize = 60;

#[derive(Debug, Clone, Copy)]
pub struct SampleCycle {
    started: Instant,
}

impl SampleCycle {
    pub fn begin() -> Self {
        Self {
            started: Instant::now(),
        }
    }

    pub fn remaining(self) -> Duration {
        remaining_sample_wait(self.started.elapsed())
    }
}

fn remaining_sample_wait(elapsed: Duration) -> Duration {
    SAMPLE_INTERVAL.saturating_sub(elapsed)
}

pub trait Collector: Send + 'static {
    fn collect(&mut self) -> RawSnapshot;
}

pub struct Sampler<C> {
    collector: C,
    previous: Option<RawSnapshot>,
    state: MonitorState,
}

impl<C: Collector> Sampler<C> {
    pub fn new(collector: C) -> Self {
        Self {
            collector,
            previous: None,
            state: MonitorState {
                history_capacity: HISTORY_CAPACITY,
                ..MonitorState::default()
            },
        }
    }

    pub fn sample(&mut self) -> MonitorState {
        let started = devlog::enabled().then(Instant::now);
        let raw = self.collector.collect();
        let snapshot = derive(self.previous.as_ref(), &raw);
        history::push(
            &mut self.state.history,
            &snapshot,
            self.state.history_capacity,
        );
        self.state.snapshot = snapshot;
        self.previous = Some(raw);
        let state = self.state.clone();

        if let Some(started) = started {
            let elapsed = started.elapsed();
            devlog::log(format_args!(
                "sampling duration={:.3}ms target={:.3}ms",
                elapsed.as_secs_f64() * 1_000.0,
                SAMPLE_INTERVAL.as_secs_f64() * 1_000.0
            ));
            if elapsed > SAMPLE_INTERVAL {
                devlog::log(format_args!(
                    "sampling overrun actual={:.3}ms target={:.3}ms",
                    elapsed.as_secs_f64() * 1_000.0,
                    SAMPLE_INTERVAL.as_secs_f64() * 1_000.0
                ));
            }
        }

        state
    }
}

#[cfg(test)]
mod tests;
