---
summary: "Defines production-path diagnostic workloads used by external performance profilers."
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
---

# Performance Diagnostic Workloads

The performance CLI selects production workloads for external profilers; it is not a parallel benchmark framework.

```text
cclover-mon perf headless [--duration <seconds> | --samples <count>]
cclover-mon perf collector <name> [--duration <seconds> | --samples <count>]
```

`perf headless` runs the normal `Collector → Sampler → MonitorState` cycle at production cadence without creating UI/presentation work. `perf collector <name>` runs the selected production collector at the same cadence and discards its result without probe formatting. Isolated collector workloads also execute the minimum production context required to preserve lifecycle, identity, bounded-state, and projection semantics. That prerequisite work is part of whole-process workload cost; use a sampling profiler when function-level attribution is required.

Collector names are the same controlled set accepted by `probe`: `cpu`, `memory`, `processes`, `network`, `network-attribution`, `disk`, `disk-attribution`, `temperatures`, `fans`, and `gpu`. Probe and performance diagnostics may use different orchestration when probe-specific detail projections would not match production sampling semantics.

The default run is unbounded for profiler attachment. `--duration` and `--samples` only terminate the run; they do not alter cadence. Diagnostic formatting, terminal output, or synthetic replacement work is excluded unless that work is the subject being measured.

Additional isolation points are justified only when they reuse the corresponding production implementation rather than creating a parallel implementation that measures different work. Isolation may remove unrelated production work, but it must not remove context that materially changes the selected collector's steady-state behavior.

Comparative experiments do not add another workload implementation. `perf-compare.ts` launches two explicit `cclover-mon` executables with the same fixed-sample `perf` workload, so this CLI remains the sole authority for what work is measured. The comparison layer owns only balanced execution order, child CPU-time measurement, and noise-aware summarization.
