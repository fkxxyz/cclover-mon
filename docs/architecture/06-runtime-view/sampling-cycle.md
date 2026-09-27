---
summary: "Defines the normal sampling cycle from native collection to UI-visible state."
viewpoint: dynamic
concerns:
  - architecture-coherence
  - performance
  - maintainability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - core
    - platform
---

# Sampling Cycle

```text
sampling deadline
   ↓
platform batch collection
   ↓
typed snapshot
   ↓
delta / aggregation / bounded history
   ↓
UI state update
```

A sampling cycle performs one coordinated batch rather than independent polling per widget or metric. Transiently unavailable native data is represented as unavailable data, not as a reason to rebuild the pipeline. The normal production cadence is one second; temperature sources participate in that same cadence rather than running widget-specific timers.

Linux temperature collection fans in peer sources within one metric collection step:

```text
one-second sampling deadline
        ↓
Linux temperature collector
   ├── cached hwmon sensor discovery/state ── read available sensors
   └── cached NVML session/device handles ── read every supported NVIDIA GPU
        ↓
merge by stable sensor identity
        ↓
core temperature snapshots/history
        ↓
presentation/UI as one parallel temperature-panel sequence
```

Source absence does not synthesize zero values. If NVML is unavailable or enumerates no NVIDIA devices, that source contributes an empty sequence. If one NVIDIA device cannot provide temperature, only that device contributes no temperature entry. hwmon-derived temperatures continue unaffected.

Event-driven native collectors may accumulate state continuously between sampling deadlines. For Linux eBPF I/O attribution, kernel programs update bounded BPF maps when I/O occurs and the sampling cycle reads already-aggregated counters. Sampling remains coordinated even when native observation itself is event-driven rather than initiated by the deadline.

Cross-sample association is identity-driven. Delta derivation, history, joins, caches, and deduplication may reuse prior state only when the current observation carries the same stable semantic identity. Matching PID, device name, display label, enumeration position, or another incidental value is insufficient unless its stability is part of the declared identity contract. If a process reuses a PID with a different `ProcessInstanceId`, it is a new entity and inherits no CPU or I/O counters from the earlier process instance.

Development observability attaches to this same path rather than creating a parallel collector:

```text
platform collection ── probe <collector> [--raw]
        │               development timing / failure reason
        ↓
core sampling ──────── development timing / overrun log
        ↓
MonitorState ───────── dump
        ↓
UI ─────────────────── screenshot
```

`probe` bypasses core derivation only to inspect one production collector directly; it does not duplicate collector logic. `--raw` exposes source counters only in this diagnostic path. `dump` uses normal sampling semantics. Because CPU, network, disk, and process CPU rates depend on deltas, it may perform the required initial sample and sampling interval before printing the derived state.

The normal sampling loop records an overrun when collection plus derivation exceeds the configured sampling interval. This is diagnostic evidence; scheduling semantics remain unchanged.
