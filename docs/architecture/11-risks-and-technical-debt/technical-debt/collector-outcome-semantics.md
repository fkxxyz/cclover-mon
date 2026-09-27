---
summary: "Tracks fragmented availability, empty-result, partial-failure, and diagnostic semantics across collectors."
viewpoint: assurance
concerns:
  - maintainability
  - portability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
---

# Collector Outcome Semantics

## Status

Active — P1.

## Problem

Collector outcome semantics are fragmented across `Option<T>`, `Vec<T>`, `Option<Vec<T>>`, `Result`, and side-channel diagnostic strings. The shared contract does not consistently distinguish valid emptiness, unsupported capability, temporary unavailability, partial success, and collection failure.

## Evidence

Some scalar metrics use `Option`; process-attribution metrics use optional vectors; network, disk, process, and temperature collection commonly return vectors whose empty state may mean either no qualifying entities or failed discovery. Linux probe availability still partly infers failure from diagnostic text such as `cannot read ...`.

## Maintenance impact

Every new collector must reinvent availability and diagnostic conventions. Core and frontends can silently collapse distinct operational states into the same value, while probe code must reconstruct semantics from strings or collector-specific knowledge.

## Governing constraint

Availability and degradation are program semantics and must be represented by typed data, not diagnostic wording. Empty data must remain distinguishable from inability to observe data whenever that distinction affects behavior or presentation.

## Resolution direction

Define the minimum shared outcome semantics needed across metric categories, including partial/degraded collection where relevant. Keep human-readable diagnostics derived from typed state rather than making diagnostic strings authoritative.

## Exit criteria

Production sampling and probes consume one explicit outcome model for availability/degradation, no program decision depends on diagnostic text, and empty collections have unambiguous semantics.
