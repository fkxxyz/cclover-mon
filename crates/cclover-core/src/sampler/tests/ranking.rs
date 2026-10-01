use std::time::{Duration, Instant};

use super::super::process::TOP_N;
use super::super::*;
use super::process_id;

#[test]
fn top_memory_keeps_only_highest_processes_in_order() {
    let processes: Vec<_> = (0..12)
        .map(|index| ProcessCounter {
            process: process_id(index, 1),
            name: format!("p{index}").into(),
            cpu_time_units: 0,
            rss_bytes: u64::from(index),
        })
        .collect();
    let current = RawSnapshot {
        processes: Collection::available(processes),
        ..RawSnapshot::default()
    };

    let out = derive(None, &current);
    let top = out.top_memory.value().unwrap();
    assert_eq!(top.len(), TOP_N);
    assert_eq!(top[0].name, "p11");
    assert_eq!(top[TOP_N - 1].name, "p4");
    assert_eq!(out.processes.by_id.len(), 12);
}

#[test]
fn top_cpu_keeps_only_highest_processes_in_order() {
    let t = Instant::now();
    let previous_processes: Vec<_> = (0..12)
        .map(|index| ProcessCounter {
            process: process_id(index, 1),
            name: format!("p{index}").into(),
            cpu_time_units: 100,
            rss_bytes: 0,
        })
        .collect();
    let current_processes: Vec<_> = previous_processes
        .iter()
        .map(|process| ProcessCounter {
            cpu_time_units: process.cpu_time_units + u64::from(process.process.pid),
            ..process.clone()
        })
        .collect();
    let previous = RawSnapshot {
        collected_at: t,
        cpu: Collection::available(CpuCounter {
            total_time_units: 1_000,
            idle_time_units: 0,
            logical_cpu_count: 1,
        }),
        processes: Collection::available(previous_processes),
        ..RawSnapshot::default()
    };
    let current = RawSnapshot {
        collected_at: t + Duration::from_secs(1),
        cpu: Collection::available(CpuCounter {
            total_time_units: 1_100,
            idle_time_units: 0,
            logical_cpu_count: 1,
        }),
        processes: Collection::available(current_processes),
        ..RawSnapshot::default()
    };

    let out = derive(Some(&previous), &current);
    let top = out.top_cpu.value().unwrap();
    assert_eq!(top.len(), TOP_N);
    assert_eq!(top[0].name, "p11");
    assert_eq!(top[TOP_N - 1].name, "p4");
    assert_eq!(out.processes.by_id.len(), 12);
}
