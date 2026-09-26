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
3. Development logging records collector/sampling duration, exceptional conditions, and sampling overruns. Normal application execution remains quiet; logging does not become a second metric transport or persistence layer.
4. Runtime screenshots validate presentation, layout, and desktop integration only. They are not evidence that collection or derivation is correct.

## Rationale

These mechanisms distinguish failures at the smallest useful boundaries:

```text
native collection correctness        → probe <collector> [--raw]
derivation correctness               → dump
runtime failure / sampling cost      → development log
presentation / placement             → screenshot
```

`dump` reuses the production sampling path and `probe` reuses production collector code, so diagnostic behavior cannot silently diverge into a second collection implementation. Timing and diagnostic reasons are attached to existing work rather than exposed as a metrics service.

## Constraints

- Diagnostic output must not introduce subprocess polling, frontend/backend IPC, or an internal JSON metric contract.
- Probe output may include platform-specific source names, raw counters, skip reasons, and errors, but platform API types must not enter the shared `MonitorState` contract.
- Disabled development diagnostics must not materially change sampling cadence or normal runtime behavior.
- A collector failure may be reported diagnostically while the normal unavailable-data semantics continue to apply.
- Sampling that exceeds its configured interval is logged as an overrun with elapsed duration and target interval.

## Deferred Until Evidence Requires It

Do not add record/replay, a debug overlay, a metrics server, persistent application logs, or separate diagnostic executables solely for anticipated debugging needs. Add one only when a concrete failure mode cannot be diagnosed adequately with the mechanisms above.
