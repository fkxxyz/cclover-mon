---
summary: "Tracks GPU metric ownership and per-field availability semantics that still rely on cross-collector coordination and compressed optional state."
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

# GPU Capability Contract

## Status

Active — P2.

## Root cause

GPU capability ownership and per-field availability semantics are not yet represented by one complete contract. Some ownership is coordinated between collectors by convention, while individual GPU fields use `Option<T>` even though future callers may need to distinguish unsupported, unimplemented, temporarily unavailable, permission-denied, and unreadable states.

## Evidence

GPU temperature is owned by the unified GPU path for AMD/NVIDIA, while generic temperature collection must avoid producing duplicate AMD GPU temperature entries. This creates a synchronization rule between otherwise separate collectors. The unified GPU snapshot also represents utilization, VRAM, temperature, power, clock, and fan capabilities as optional values; absence currently means only "not displayable now" and does not preserve why the field is absent.

## Governing constraint

Each physical GPU metric has one unambiguous ownership path, and capability/availability distinctions required by presentation, diagnostics, fallback, or recovery logic must survive the platform boundary as typed semantics rather than driver-name coordination or overloaded `None`.

## Scope discovery

Trace every GPU-related native source and every generic sensor source that can describe the same physical GPU. Identify deduplication/ownership decisions, fallback behavior, and all consumers of optional GPU fields across core, history, presentation, diagnostics, HTTP transport, and future platform backends. Include Intel, Windows, alternative Linux drivers, and partial native-source failure as expected change cases.

## Maintenance consequence

Adding another GPU vendor/backend or fallback source can require edits in multiple collectors merely to prevent duplication. A backend capability change can make a metric disappear or duplicate without a single authority deciding ownership. If UI or diagnostics later need to explain why a field is missing, existing `Option<T>` values cannot do so without adding contextual side channels or changing the model again.

## Repair direction

First establish one device-level ownership/deduplication rule that native sources can feed without knowing sibling collector policy. Introduce richer per-field capability/availability types only when a real consumer needs those distinctions; do not build a speculative generic capability framework before that pressure exists.

## Exit criteria

For all supported GPU/native-source combinations discovered in scope, one physical GPU metric cannot be emitted twice or suppressed solely because two collectors must remember matching exclusion rules. Missing GPU fields have typed semantics sufficient for every current consumer that needs to distinguish absence causes, with deterministic tests covering ownership, fallback, and partial failure.
