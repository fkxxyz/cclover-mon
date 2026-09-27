---
summary: "Tracks collector outcome semantics that are typed at collection time but compressed before derived state and history."
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

Collector outcome semantics are explicit in `RawSnapshot` through `Collection<T>`, but derivation compresses them back into `Option<T>`, `Vec<T>`, and `Option<Vec<T>>`. Valid empty, degraded partial, and unavailable observations therefore become indistinguishable after the raw boundary.

## Evidence

`RawSnapshot` stores every metric as `Collection<T>`, while `SystemSnapshot` stores weak containers without collection status. `core::sampler::derive` repeatedly calls `.value()` and `unwrap_or_default()`, discarding `Degraded` and unavailability reasons. `core::history::push` then treats derived empty network, disk, and temperature sequences as authoritative absence and removes missing histories, so unavailable or partial observations can mutate history as if the entities were known to be absent.

## Maintenance impact

Stateful consumers must reconstruct availability semantics from container shape or cannot recover them at all. New frontends, recovery behavior, and partial collectors can therefore disagree about empty, degraded, and unavailable data, while history correctness depends on distinctions that have already been erased.

## Governing constraint

Observation completeness is program semantics. Valid empty, degraded partial, and unavailable observations must remain distinguishable through every derivation, history, or presentation-facing state that makes decisions based on that distinction. Unavailable data must not be treated as observed absence, and omitted entities in a degraded observation must not be treated as confirmed removal.

## Scope discovery

Trace every `Collection<T>` from platform collection through `RawSnapshot`, derivation, `SystemSnapshot`, history, presentation, Web transport, CLI, and UI consumers. Search for `.value()`, `into_value()`, `Option`/`Vec` projection, default-empty conversion, and state eviction based only on entity absence.

## Resolution direction

Preserve typed outcome semantics through derived state and make history updates status-aware. Keep human-readable diagnostics derived from typed state rather than using container shape or text as an implicit status channel.

## Exit criteria

The full discovered path preserves the distinctions needed by downstream behavior; unavailable observations do not erase history, degraded partial observations do not imply removal of omitted entities, valid empty observations remain representable, and recovery starts from a fresh baseline where required. Tests cover these states through derivation and history rather than only at the raw collection boundary.
