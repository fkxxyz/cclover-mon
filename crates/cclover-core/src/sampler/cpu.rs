use std::collections::HashMap;

use super::status::{combine_status, map_status};
use crate::model::*;

pub(super) fn derive_cpu(
    previous: Option<&RawSnapshot>,
    current: &RawSnapshot,
    current_processes: &[ProcessCounter],
) -> (Collection<f64>, Collection<Vec<f64>>) {
    let process_status = combine_status(&current.cpu, &current.processes, ()).status();
    let Some(new) = current.cpu.value() else {
        let reason = match current.cpu.status() {
            CollectionStatus::Unavailable(reason) => reason,
            _ => unreachable!("observable collection must expose a value"),
        };
        return (
            Collection::Unavailable(reason),
            process_status.wrap(Vec::new()),
        );
    };

    let Some((previous, old)) =
        previous.and_then(|snapshot| snapshot.cpu.value().map(|counter| (snapshot, counter)))
    else {
        let values = vec![0.0; current_processes.len()];
        return (map_status(&current.cpu, 0.0), process_status.wrap(values));
    };

    let total = new.total_time_units.saturating_sub(old.total_time_units);
    if total == 0 {
        let values = vec![0.0; current_processes.len()];
        return (map_status(&current.cpu, 0.0), process_status.wrap(values));
    }

    let idle = new.idle_time_units.saturating_sub(old.idle_time_units);
    let percent = (100.0 * (total.saturating_sub(idle)) as f64 / total as f64).clamp(0.0, 100.0);
    let old_processes: HashMap<ProcessInstanceId, &ProcessCounter> = previous
        .processes
        .value()
        .into_iter()
        .flatten()
        .map(|process| (process.process, process))
        .collect();
    let scale = new.logical_cpu_count.max(1) as f64 * 100.0 / total as f64;
    let values = current_processes
        .iter()
        .map(|process| {
            let delta = old_processes.get(&process.process).map_or(0, |old| {
                process.cpu_time_units.saturating_sub(old.cpu_time_units)
            });
            delta as f64 * scale
        })
        .collect();

    (
        map_status(&current.cpu, percent),
        process_status.wrap(values),
    )
}
