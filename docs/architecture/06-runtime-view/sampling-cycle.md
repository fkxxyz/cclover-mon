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

A sampling cycle performs one coordinated batch rather than independent polling per widget or metric. Transiently unavailable native data is represented as unavailable data, not as a reason to rebuild the pipeline. The normal production cadence is one second; temperature and fan sources participate in that same cadence rather than running widget-specific timers.

Sampling cadence is measured from the start of each sampling cycle. Core owns the cadence policy: after a cycle, callers wait only for the portion of the one-second interval not already consumed by collection and derivation. If a cycle overruns the interval, the next cycle may begin immediately rather than accumulating additional delay. GUI and CLI adapters may use different waiting mechanisms, but they must consume this shared policy rather than reimplement interval arithmetic. Execution adapters such as `app` and `cli` must not depend directly on `SAMPLE_INTERVAL`; direct interval use is reserved for non-scheduler semantics such as cache freshness or explicit diagnostic sampling where no next-cycle scheduling decision is being made.

The first successful observation is also a UI-visible snapshot; startup does not wait for a second sampling deadline merely to establish rate baselines. Data that is knowable from the current observation alone, such as memory, GPU telemetry, temperatures, process identity/memory, disk identity, and network-interface identity, is published immediately. A derived rate whose source is currently available but has no comparable prior counter is initialized to zero for that baseline cycle while preserving the observed entity. This applies equally when a source or entity first appears after startup. Source unavailability remains distinct: a collector that cannot provide the current metric is still represented as unavailable rather than as zero.

Linux GPU collection fans in vendor-native sources within one metric collection step:

```text
one-second sampling deadline
        ↓
Linux GPU collector
   ├── AMD amdgpu DRM/sysfs + device hwmon
   └── cached NVML session/device handles
        ↓
normalize to stable GpuId + optional common fields
        ↓
core GPU snapshots/history
        ↓
presentation/UI as one GPU card per device
```

Source absence does not synthesize zero values. Missing GPU fields remain `None`; readable fields on the same device stay available. Generic temperature collection runs independently for non-GPU sensors and GPU families not represented by the GPU collector.

Windows hardware telemetry performs stable discovery outside the sampling hot path. Each cycle reads each known CPU/Super-I/O/EC source once and projects the resulting observation into metric-specific temperature and fan collections. Those projections do not reopen PawnIO, rediscover the same chip, or independently serialize the same bus. Each independent source keeps its own lifecycle; a source transport/session failure enters bounded retry for that source without resetting unrelated sources, while per-observation validation failures and transient bus contention do not discard an otherwise live source session.

Event-driven native collectors may accumulate state continuously between sampling deadlines. For Linux eBPF I/O attribution, kernel programs update bounded BPF maps when I/O occurs and the sampling cycle reads already-aggregated counters. Sampling remains coordinated even when native observation itself is event-driven rather than initiated by the deadline.

Cross-sample association is identity-driven. Delta derivation, history, joins, caches, and deduplication may reuse prior state only when the current observation carries the same stable semantic identity. GPU utilization, VRAM-used, and temperature histories are independently bounded and keyed by `GpuId`; presentation binds each current value to its corresponding history, with VRAM using the device's current total VRAM as the fixed graph range. Matching PID, device name, display label, enumeration position, or another incidental value is insufficient unless its stability is part of the declared identity contract. If a process reuses a PID with a different `ProcessInstanceId`, it is a new entity and inherits no CPU or I/O counters from the earlier process instance.

Core also owns process-domain assembly and dashboard ranking semantics. After per-process CPU and disk/network attribution counters become current rates, core joins process metadata, memory, CPU, per-disk I/O, and per-interface network I/O by `ProcessInstanceId` into the complete `ProcessDomainSnapshot`. Identities reported only by attribution remain in the domain with missing metadata, and capability status remains explicit. Only after this complete join does core derive the existing card projections: Top CPU and memory rows, plus the three highest process-I/O rows per stable `DiskId` / `NetworkId`, ranked by the sum of the two directional rates. Presentation may omit zero-rate rows from display, but frontend code must not redo attribution joins, ranking, or platform lookup.

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

`probe` bypasses core derivation only to inspect one production collector directly; it does not duplicate collector logic. A targeted Windows hardware probe initializes and samples only the source required by that metric rather than executing the complete production hardware batch. `--raw` exposes source counters only in this diagnostic path. `dump` uses normal sampling semantics. Because CPU, network, disk, and process CPU rates depend on deltas, it may perform the required initial sample and sampling interval before printing the derived state.

The normal sampling loop records an overrun when collection plus derivation exceeds the configured sampling interval. This is diagnostic evidence; scheduling semantics remain unchanged.
