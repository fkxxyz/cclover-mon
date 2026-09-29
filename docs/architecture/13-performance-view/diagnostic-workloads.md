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

`perf headless` runs the normal `Collector → Sampler → MonitorState` cycle at production cadence without creating UI/presentation work. `perf collector <name>` runs one production collector at the same cadence and discards its result without probe formatting.

Collector names are the same controlled set accepted by `probe`: `cpu`, `memory`, `processes`, `network`, `network-attribution`, `disk`, `disk-attribution`, `temperatures`, and `gpu`. Attribution-specific names isolate userspace BPF-map sampling from whole-interface/device collection.

The default run is unbounded for profiler attachment. `--duration` and `--samples` only terminate the run; they do not alter cadence. Diagnostic formatting, terminal output, or synthetic replacement work is excluded unless that work is the subject being measured.

Additional isolation points are justified only when they reuse the corresponding production implementation rather than creating a parallel implementation that measures different work.
