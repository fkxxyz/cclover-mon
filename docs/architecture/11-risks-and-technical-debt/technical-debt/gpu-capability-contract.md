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

Resolved.

## Root cause

GPU temperature ownership used to be coordinated between sibling collectors by convention: generic hwmon discovery had to know which GPU families another collector already owned. Individual GPU fields still use `Option<T>`, but no current consumer requires distinct unsupported, unimplemented, temporarily unavailable, permission-denied, or unreadable states, so richer per-field availability is intentionally deferred until that distinction has observable behavior.

## Evidence

The Linux backend now reconciles platform-private GPU and hwmon temperature observations by canonical physical-device identity. Generic hwmon discovery has no `amdgpu` exclusion; an actually discovered GPU claims matching temperature observations at batch composition, unmatched sensors remain generic, and an existing vendor GPU temperature wins over hwmon fallback. Deterministic tests cover matching ownership, source priority, unmatched sensors, and unavailable GPU collection. GPU snapshot fields remain optional because all current consumers only need the distinction between a displayable value and no displayable value.

## Governing constraint

Each physical GPU metric has one unambiguous ownership path, and capability/availability distinctions required by presentation, diagnostics, fallback, or recovery logic must survive the platform boundary as typed semantics rather than driver-name coordination or overloaded `None`.

## Scope discovery

Trace every GPU-related native source and every generic sensor source that can describe the same physical GPU. Identify deduplication/ownership decisions, fallback behavior, and all consumers of optional GPU fields across core, history, presentation, diagnostics, HTTP transport, and future platform backends. Include Intel, Windows, alternative Linux drivers, and partial native-source failure as expected change cases.

## Historical maintenance consequence

Before resolution, adding another GPU vendor/backend or fallback source could require edits in multiple collectors merely to prevent duplication, and a backend capability change could make a metric disappear or duplicate without a single authority deciding ownership. A future UI or diagnostic that needs to explain why a GPU field is missing would require a richer field contract; that is a new requirement trigger, not current debt.

## Resolution

Device-level ownership is centralized at the Linux backend composition point. Native collectors report observations plus platform-private physical identity and do not encode sibling-collector exclusion policy. The shared `GpuSnapshot` contract remains unchanged. Richer per-field capability/availability types are not introduced without a real consumer that needs absence causes; doing so now would create speculative infrastructure rather than remove current maintenance cost.

## Exit criteria

Met for current supported sources: GPU temperature ownership no longer depends on sibling collectors remembering matching exclusion rules, and deterministic tests cover ownership, fallback priority, unmatched sensors, and GPU-source unavailability. Current consumers do not distinguish GPU field absence causes, so `Option<T>` is sufficient for current semantics. Reopen this concern only when a concrete consumer needs typed absence causes or a new native source requires additional cross-source reconciliation.
