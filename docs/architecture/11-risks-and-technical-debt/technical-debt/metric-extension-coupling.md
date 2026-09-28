---
summary: "Tracks the growing edit radius caused by metric-specific behavior spread across central switchboards."
viewpoint: assurance
concerns:
  - maintainability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
---

# Metric Extension Coupling

## Status

Resolved.

## Problem

Metric-specific behavior is distributed through multiple central switchboards. Adding or materially changing one metric therefore requires coordinated edits across unrelated metric dispatch and presentation paths instead of remaining local to the metric's responsibility.

## Evidence

Current extension points include `ProbeKind` naming/parsing, Linux `Backend::probe`, `collect_for_perf`, normal backend snapshot composition, core derivation/history, renderer-neutral presentation, desktop layout/render dispatch, and CLI help/dump behavior. Large functions such as probe dispatch or derivation are symptoms of this wider edit-radius problem, not separate debts.

## Maintenance impact

Metric count increases the number of places that must remain synchronized. A partial change can leave probe, perf, normal sampling, history, UI, or diagnostics semantically inconsistent. Refactoring individual long functions without reducing the number of authorities does not resolve the debt.

## Governing constraint

A metric-specific change should be local to the smallest responsible modules. Shared orchestration may enumerate metrics where composition genuinely requires it, but static metadata and metric behavior must not be redundantly encoded across unrelated central switches.

## Resolution

`ProbeKind` now has one metadata definition for canonical CLI names, accepted aliases, and probe-only follow-up sampling. CLI parsing, error text, and help derive from that authority instead of maintaining separate collector-name lists.

Linux and Windows each now have one `ProbeKind` → production-collector dispatch used by both `probe` and `perf collector`. The resulting typed `ProbeSample` is formatted separately from collection, so diagnostic presentation no longer duplicates collector selection. Linux attribution probes retain their second-sample behavior without making perf benchmarks sleep between samples.

Core derivation was re-evaluated rather than mechanically split: top-level `derive` remains explicit snapshot composition, while CPU, network, disk, process-disk, and process-network derivation already live in focused metric helpers. Presentation and frontend metric branches likewise remain where they express real product semantics. No generic plugin/registry framework was introduced.

The platform-boundary architecture now requires one development-collector metadata authority and shared probe/perf collector dispatch, preventing the removed duplication from reappearing.

## Exit criteria

Adding a new metric no longer requires synchronized edits to multiple unrelated dispatch tables solely to teach the system that the metric exists; remaining cross-layer edits correspond to real product semantics rather than duplicated control metadata.

This criterion is met: a new diagnostic collector is declared once in `ProbeKind`, connected once per supporting platform to its production collector, and then receives only the model/derivation/presentation/frontend work required by its actual product semantics.
