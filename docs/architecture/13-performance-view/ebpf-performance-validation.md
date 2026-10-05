---
summary: "Defines separate event-path and userspace-sampling performance validation for Linux eBPF attribution."
viewpoint: performance
concerns:
  - performance
activities:
  - orient
  - change
  - assess
facets:
  area:
    - platform
---

# eBPF Performance Validation

Measure Linux eBPF attribution in two independent cost domains:

- **event-path overhead** — compare controlled high-I/O workloads with attribution disabled and enabled, using whole-process/system CPU plus workload throughput/latency. `CCLOVER_MON_DISABLE_EBPF_IO=1` is the diagnostic A/B switch for this measurement, not a second collector implementation.
- **sampling overhead** — use `perf collector disk-attribution` and `perf collector network-attribution` as production-faithful attribution workloads. They prepare the same complete active-process context used by normal sampling so stale-map retirement remains equivalent. Whole-process cost therefore includes that prerequisite process snapshot; use a sampling profiler to attribute BPF-map iteration, native-ID resolution, and output shaping specifically.

Absence of userspace polling does not prove low overhead. Hook frequency, map contention, per-event work, and cardinality can dominate. Validate ordinary desktop traffic and deliberately high event-rate workloads, while keeping map state bounded.

Correctness coverage and byte semantics are defined by the Linux eBPF runtime Views; performance evidence must use workloads consistent with those semantics.
