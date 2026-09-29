---
summary: "Tracks coupled initialization, retry, and probe behavior across independent Windows hardware-telemetry sources."
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

# Windows Hardware Source Lifecycle

Priority: P2.

## Root cause

The Windows hardware-telemetry runtime owns independent PawnIO-backed sources behind one aggregate runtime state. Initialization, retry, and probe selection are therefore coupled more tightly than the physical sources themselves: requesting one metric can initialize unrelated module sessions, while runtime failures do not yet have a uniform per-source recovery lifecycle.

## Evidence

The current hardware runtime coordinates Intel MSR temperature and LPC/Super-I/O fan collection under one collector. Normal sampling benefits from one aggregate batch, but targeted temperature and fan probes enter that same aggregate path. Source module sessions are independent, yet retry and stable-failure policy is primarily expressed at aggregate initialization rather than as a reusable source lifecycle.

## Governing constraint

Windows hardware telemetry may have one subsystem owner and one coordinated sampling entry point, but each independently failing hardware source must own its own initialization and recovery state. A targeted probe must not require unrelated hardware sources to initialize or perform I/O.

## Scope discovery

Review every Windows hardware-telemetry source owned below the hardware runtime, including current PawnIO module sessions and future CPU, Super-I/O, EC, voltage, power, or board-specific sources. Trace initialization, module loading, sampling, failure classification, retry/backoff, diagnostics, targeted probe paths, and aggregate batch composition for each source.

## Maintenance consequence

As hardware sources are added, one source's capability, failure, or retry behavior can force changes in aggregate runtime control flow and can affect diagnostics or probes for unrelated metrics. Recovery policy can also diverge between sources if each new path handles post-initialization failures ad hoc.

## Repair direction

Keep aggregate hardware-runtime ownership, but give each independent source a small explicit lifecycle such as uninitialized, ready, retry-at, or stable-unavailable. Compose those source observations into the normal hardware batch, while targeted probes activate only the requested source. Share lifecycle policy where semantics are identical; do not introduce a generic sensor model.

## Exit criteria

Every independent Windows hardware source can initialize, fail, retry, and become stably unavailable without changing unrelated source state. Targeted probes initialize and sample only their required sources. Deterministic tests cover lifecycle transitions and aggregate partial-failure composition for the discovered source set.
