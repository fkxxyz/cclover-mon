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

Active — P1.

## Problem

Metric-specific behavior is distributed through multiple central switchboards. Adding or materially changing one metric therefore requires coordinated edits across unrelated metric dispatch and presentation paths instead of remaining local to the metric's responsibility.

## Evidence

Current extension points include `ProbeKind` naming/parsing, Linux `Backend::probe`, `collect_for_perf`, normal backend snapshot composition, core derivation/history, renderer-neutral presentation, desktop layout/render dispatch, and CLI help/dump behavior. Large functions such as probe dispatch or derivation are symptoms of this wider edit-radius problem, not separate debts.

## Maintenance impact

Metric count increases the number of places that must remain synchronized. A partial change can leave probe, perf, normal sampling, history, UI, or diagnostics semantically inconsistent. Refactoring individual long functions without reducing the number of authorities does not resolve the debt.

## Governing constraint

A metric-specific change should be local to the smallest responsible modules. Shared orchestration may enumerate metrics where composition genuinely requires it, but static metadata and metric behavior must not be redundantly encoded across unrelated central switches.

## Resolution direction

Consolidate metric metadata where it is truly common, move metric-specific derivation/diagnostic behavior toward the owning responsibility, and retain explicit composition rather than introducing a generic plugin framework that erases meaningful metric differences.

## Exit criteria

Adding a new metric no longer requires synchronized edits to multiple unrelated dispatch tables solely to teach the system that the metric exists; remaining cross-layer edits correspond to real product semantics rather than duplicated control metadata.
