---
summary: "Records the minimal development observability used to diagnose metric collection, derivation, and UI rendering without introducing a parallel monitoring subsystem."
viewpoint: decision
concerns:
  - performance
  - maintainability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
---

# ADR 003: Minimal Development Observability

## Decision

Development observability consists of a small set of mechanisms with distinct responsibilities:

1. `cclover-mon dump` runs the normal `Collector → Sampler → MonitorState` path without starting the GUI, prints one human-readable derived state, then exits. Metrics that require deltas are obtained from two normal samples separated by the sampling interval.
2. `cclover-mon probe <collector>` invokes one production collector in isolation and reports its result, elapsed time, and diagnostic failure/skip reasons. `--raw` additionally exposes the native counters or source values needed to verify delta/rate derivation. This is a diagnostic view over the existing collector, not a second collector implementation.
3. `cclover-mon perf headless` and `cclover-mon perf collector <collector>` expose production workloads without UI or diagnostic formatting so external profilers can attribute runtime cost. They retain the normal sampling cadence and discard results rather than print each iteration. Optional `--duration` and `--samples` limits terminate the diagnostic run without changing the workload cadence.
4. Development logging records collector/sampling duration, exceptional conditions, and sampling overruns. Normal application execution remains quiet; logging does not become a second metric transport or persistence layer.
5. Runtime screenshots validate presentation, layout, and desktop integration only. They are not evidence that collection or derivation is correct.

## Rationale

These mechanisms distinguish failures at the smallest useful boundaries:

```text
native collection correctness        → probe <collector> [--raw]
derivation correctness               → dump
collector runtime cost               → perf collector <collector>
collection + core runtime cost       → perf headless
runtime failure / sampling cost      → development log
presentation / placement             → screenshot
```

`dump` and `perf headless` reuse the production sampling path. `probe` and `perf collector` reuse the same production collector code. The performance commands deliberately omit probe formatting because diagnostic string construction is not part of collector runtime cost. Diagnostic behavior therefore cannot silently diverge into a second collection implementation, while performance measurements can isolate the intended workload.

## Constraints

- Diagnostic output must not introduce subprocess polling, frontend/backend IPC, or an internal JSON metric contract.
- Probe output may include platform-specific source names, raw counters, skip reasons, and errors, but platform API types must not enter the shared `MonitorState` contract.
- Performance commands must remain workload selectors for external profilers. They must not grow a second sampling engine, duplicate collector logic, or substitute synthetic collection for production collection.
- Performance commands keep the production sampling cadence. Bounded-run options control termination only.
- Disabled development diagnostics must not materially change sampling cadence or normal runtime behavior.
- A collector failure may be reported diagnostically while the normal unavailable-data semantics continue to apply.
- Sampling that exceeds its configured interval is logged as an overrun with elapsed duration and target interval.

## Deferred Until Evidence Requires It

Do not add record/replay, a debug overlay, a metrics server, persistent application logs, or separate diagnostic executables solely for anticipated debugging needs. Add one only when a concrete failure mode cannot be diagnosed adequately with the mechanisms above.
