---
summary: "Defines how cclover-mon performance is measured, diagnosed, optimized, and regression-checked."
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

Performance work follows an evidence chain from whole-process cost to the dominant source-level cause:

```text
baseline
  ↓
subsystem attribution
  ↓
function-level profiling
  ↓
source-level cause
  ↓
minimal optimization
  ↓
same-workload remeasurement
```

## Measurement Layers

- **Process baseline** — use process-level tools such as `ps`, `top`, and `perf stat` to establish CPU time, CPU percentage, memory footprint, wakeups, or other metrics relevant to the reported problem.
- **Subsystem attribution** — use production-path diagnostics such as `probe <collector> [--raw]` and sampling timing logs to distinguish collection, derivation, and presentation cost.
- **Function attribution** — on Linux, use sampling profilers such as `perf record -g` and `perf report` to identify dominant functions and call paths across the complete process.
- **Source attribution** — connect measured hot functions to the responsible algorithm, allocation, IO, data movement, layout, text, or rendering path before selecting an optimization.

## Optimization Rules

- Establish a baseline before optimizing.
- Optimize the dominant measured cost rather than code that merely appears inefficient.
- Prefer the smallest change that removes the measured source of cost without changing product semantics unnecessarily.
- Collector timing is local evidence; whole-process profiling is required when cost may also come from core, UI, allocator, renderer, driver, or native-library work.
- Select metrics that match the problem. CPU time, wakeups, allocation, RSS, and data movement are independent dimensions and do not all need measurement for every investigation.

## Validation

Remeasure after the change under the same workload, sampling interval, build profile, and comparable runtime conditions used for the baseline. A performance optimization is accepted only when measurement demonstrates the intended improvement without violating existing correctness or architecture constraints.
