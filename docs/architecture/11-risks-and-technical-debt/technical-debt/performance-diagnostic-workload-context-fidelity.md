---
summary: "Records that isolated collector performance workloads can omit production lifecycle context and misattribute steady-state cost."
viewpoint: assurance
concerns:
  - performance
  - maintainability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - core
    - platform
    - whole-system
---

# Performance Diagnostic Workload Context Fidelity

**Priority:** Medium

## Root cause

`perf collector <name>` isolates a production collector implementation but does not have a governing contract for preserving every piece of production sampling context that can affect that collector's lifecycle and steady-state work. A collector can therefore execute the same local function while receiving materially different surrounding state from the normal `Collector → Sampler → MonitorState` path.

## Primary cost dimension

Performance diagnosis and optimization effort.

## Current cost

Maintainers can rank the wrong hotspot, spend investigation time explaining contradictory measurements, or optimize an artificial diagnostic workload instead of the production cost. The burden recurs whenever a collector's work depends on state maintained or supplied by the complete sampling cycle.

## Evidence

During Linux CPU investigation, the isolated `network-attribution` workload initially appeared to consume about 2% of one core and looked like a dominant steady-state cost. Inspection of the already-observed execution contract showed that isolated attribution collection lacked the complete active-process context used by normal sampling to retire stale eBPF attribution entries. Production-path measurement then showed steady-state network attribution around 0.39 ms per sample and disk attribution around 0.83 ms per sample, materially changing optimization priority. The diagnostic workload had measured real collector code, but not equivalent lifecycle state.

## Cost mechanism

Isolation removes work in order to improve attribution, but context-dependent collectors can also lose inputs that bound or retire their internal state. Their diagnostic data set can then differ from production in size or lifecycle. Because the command still presents itself as a production collector workload, the semantic difference is easy to interpret as a production performance result rather than a diagnostic limitation.

## Reachable better state

Each isolated performance workload either preserves the production context required to make the measured collector's work semantically equivalent, or explicitly declares that faithful isolation is unavailable and directs maintainers to a broader production-path workload. Context preparation should reuse production authorities rather than build a parallel benchmark implementation.

## Governing constraint

A production-path performance diagnostic may remove unrelated work, but it must not change the lifecycle, identity, bounded-state, or other context semantics that materially determine the cost of the work being measured.

## Scope discovery

Review every `perf collector` workload and identify inputs or lifecycle state that its production invocation obtains from the complete collection cycle. Include active-process identity, discovery caches, topology or native-identity resolution, failure/backoff state, shared hardware observations, attribution-map retirement, and any future collector whose steady-state cost depends on preceding collection. Compare isolated invocation with the corresponding production call site rather than assuming function reuse alone establishes workload fidelity.

## Repair direction

Add the smallest shared preparation/context mechanism needed for collectors whose isolated workload currently changes material production semantics. Prefer reusing the production context authority. Where constructing that context would require executing most of the normal cycle, make the diagnostic boundary explicit and use `perf headless` or another faithful production-path isolation point instead of synthesizing a second collector environment.

Do not solve this by copying production lifecycle logic into the performance CLI or by adding collector-specific fixture logic that itself becomes a second authority.

## Exit criteria

- Scope discovery covers every currently supported `perf collector` workload.
- Each context-dependent collector either receives production-equivalent context or is explicitly excluded from claims of faithful isolated steady-state measurement.
- Linux disk/network attribution diagnostics preserve the same stale-process retirement semantics as production when used for steady-state sampling-cost claims.
- Deterministic tests or another cheap executable proof detect future drift between required production context and isolated diagnostic setup.
- Performance documentation states the resulting trustworthy isolation boundary without maintaining a second list that can diverge from executable behavior.
- Repeating the attribution measurement no longer produces a material priority inversion solely because the diagnostic workload retained state that production would retire.
