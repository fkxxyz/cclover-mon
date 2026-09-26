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

A sampling cycle performs one coordinated batch rather than independent polling per widget or metric. Transiently unavailable native data is represented as unavailable data, not as a reason to rebuild the pipeline.

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
