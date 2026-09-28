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

Process disk/network attribution exposes the user-visible consequence directly: an eBPF permission failure becomes `None` after derivation, while successful collection with no active process I/O becomes an empty sequence. Presentation therefore cannot distinguish “no active process I/O” from “process I/O unavailable”, and the UI currently renders both as an empty attribution region.

## Maintenance impact

Stateful consumers must reconstruct availability semantics from container shape or cannot recover them at all. New frontends, recovery behavior, and partial collectors can therefore disagree about empty, degraded, and unavailable data, while history correctness depends on distinctions that have already been erased.

## Governing constraint

Observation completeness is program semantics. Valid empty, degraded partial, and unavailable observations must remain distinguishable through every derivation, history, or presentation-facing state that makes decisions based on that distinction. Unavailable data must not be treated as observed absence, and omitted entities in a degraded observation must not be treated as confirmed removal.

## Scope discovery

Trace every `Collection<T>` from platform collection through `RawSnapshot`, derivation, `SystemSnapshot`, history, presentation, Web transport, CLI, and UI consumers. Search for `.value()`, `into_value()`, `Option`/`Vec` projection, default-empty conversion, and state eviction based only on entity absence.

## Resolution direction

Preserve typed outcome semantics through derived state and make history updates status-aware. The minimum UI-facing contract is `Available(T)` versus `Unavailable(reason)`: valid empty payloads remain available observations, while permission, unsupported, disabled, or temporary failure states remain explicit. Presentation maps those platform-neutral states into renderer-neutral UI semantics; frontends do not inspect eBPF, ETW, errno, Win32 status codes, or diagnostic strings. Optional child capabilities remain independently renderable so their failure does not suppress an available parent metric.

Do not introduce a separate partial/degraded presentation state until a concrete downstream behavior requires it. Partial fields may remain represented inside an available payload where that is sufficient; extend the state model only when consumers need to distinguish partial observation as a first-class condition.

## Exit criteria

The full discovered path preserves the distinctions needed by downstream behavior; unavailable observations do not erase history, degraded partial observations do not imply removal of omitted entities, valid empty observations remain representable, and recovery starts from a fresh baseline where required. Native and Web presentation can distinguish at least `Available(empty)` from `Unavailable(reason)` for independently failing capabilities such as process disk/network attribution, and render child-capability failure without hiding an available parent metric. Tests cover these states through derivation, history, transport, and presentation rather than only at the raw collection boundary.
