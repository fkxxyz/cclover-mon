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

For baseline-versus-candidate CPU comparisons, use the repository-owned comparison harness rather than reconstructing shell-level A/B orchestration:

```text
bun perf-compare.ts --baseline <baseline-executable> --candidate <candidate-executable> headless --samples <count> [--pairs <even-count>]
bun perf-compare.ts --baseline <baseline-executable> --candidate <candidate-executable> collector <name> --samples <count> [--pairs <even-count>]
```

The caller supplies already-built executables; comparison does not own Git checkout or build policy. Before measurement it records the SHA-256 identity of each executable and verifies the same identity again after all runs, failing the comparison if either executable changed. It requires fixed sample counts so both sides perform the same number of production sampling cycles. Runs use at least six pairs and alternate baseline-first/candidate-first order with an even pair count. The harness records child user, system, and total CPU time and summarizes paired relative deltas by median and interquartile range. It reports `candidate consistently lower` or `candidate consistently higher` only when every paired total-CPU delta agrees in direction; any directional disagreement is `inconclusive`. The interquartile range remains descriptive spread rather than a significance test. This directional evidence does not by itself establish that an observed difference is materially large enough to justify accepting a change, and it does not replace correctness or maintainability judgment.
