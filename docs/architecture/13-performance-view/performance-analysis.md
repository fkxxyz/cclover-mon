---
summary: "Indexes how cclover-mon performance is measured, diagnosed, optimized, and regression-checked."
viewpoint: performance
concerns:
  - performance
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
    - core
    - platform
    - ui
    - native-bridge
---

# Performance Analysis

Performance work follows one evidence chain:

```text
baseline
  ↓
subsystem attribution
  ↓
function/source attribution
  ↓
minimal optimization
  ↓
same-workload remeasurement
```

Detailed Views:

- [Measurement Method](measurement-method.md) — baseline, attribution, optimization, and acceptance rules.
- [Diagnostic Workloads](diagnostic-workloads.md) — production-path `perf` CLI workloads for external profilers.
- [eBPF Performance Validation](ebpf-performance-validation.md) — separate event-path and userspace-sampling measurements for Linux attribution.

Performance claims require comparable before/after evidence. Correctness diagnostics and architecture constraints are inputs to performance work, not substitutes for measurement.
