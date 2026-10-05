---
summary: "Defines baseline, attribution, optimization, and remeasurement rules for performance work."
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

# Performance Measurement Method

Start with the metric that matches the reported problem: CPU time, CPU percentage, memory footprint, wakeups, allocation, or data movement. Establish a process baseline with tools such as `ps`, `top`, and `perf stat`.

Use production-path diagnostic workloads to remove layers without replacing the work under investigation. A diagnostic may remove unrelated work, but it must preserve prerequisite lifecycle and identity context that materially determines the measured work's steady-state cost. Then use sampling profilers such as `perf record -g` / `perf report` to attribute cost to functions and connect measured hot paths to the responsible algorithm, I/O, allocation, data movement, layout, text, renderer, driver, or native-library behavior.

Optimize the dominant measured cause with the smallest change that preserves product semantics. Local collector timing is insufficient when cost may originate elsewhere in the process.

Repeated native sampling should reuse discovery and long-lived handles when the native interface permits it. Stable topology and identity resolution are lifecycle concerns, not work to repeat for every field or attribution row. Event-driven state remains bounded and retires dead-process entries so per-sample traversal scales with current/recent activity rather than process-lifetime history.

Remeasure under the same workload, sampling interval, build profile, and comparable runtime conditions. Accept an optimization only when the intended improvement is measured without violating correctness or architecture constraints.
