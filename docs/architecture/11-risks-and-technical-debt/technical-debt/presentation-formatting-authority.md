---
summary: "Tracks duplicated human-readable formatting semantics outside the renderer-neutral presentation layer."
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

# Presentation Formatting Authority

## Status

Active — P2.

## Problem

Human-readable metric formatting has more than one authority. The renderer-neutral presentation layer owns shared formatting, while `cclover-mon dump` independently implements byte, percentage, and unavailable-value formatting with different rules.

## Evidence

`presentation.rs` exposes common format helpers and unavailable semantics; `cli.rs` defines separate `bytes` and `percent` helpers and prints different unavailable text.

## Maintenance impact

A future terminal frontend or additional diagnostic output can create a third formatting path. Product-visible units, precision, and unavailable semantics then drift depending on surface.

## Governing constraint

Human-facing metric semantics shared by frontends should have one renderer-neutral authority. Performance measurement paths must remain free to avoid formatting work when formatting is not part of the measured workload.

## Resolution direction

Reuse or extract presentation-owned formatting semantics for human-readable CLI/frontend output while preserving raw/benchmark paths where presentation work is intentionally excluded.

## Exit criteria

Equivalent human-facing values use one shared formatting authority across desktop, CLI, and future terminal presentation surfaces.
