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
same-workload paired comparison
```

Detailed Views:

- [Measurement Method](measurement-method.md) — baseline, attribution, optimization, and acceptance rules.
- [Diagnostic Workloads](diagnostic-workloads.md) — production-path native `perf` workloads plus the active-Web launcher for external profilers and fixed-work comparison.
- [eBPF Performance Validation](ebpf-performance-validation.md) — separate event-path and userspace-sampling measurements for Linux attribution.

Performance claims require comparable before/after evidence. Repository-owned `perf-compare.ts` standardizes fixed-work baseline/candidate CPU comparisons while each workload remains owned by its production-faithful authority: the native `perf` CLI for headless/collector isolation and `perf-web.ts` for active Web delivery. Correctness diagnostics and architecture constraints are inputs to performance work, not substitutes for measurement.
