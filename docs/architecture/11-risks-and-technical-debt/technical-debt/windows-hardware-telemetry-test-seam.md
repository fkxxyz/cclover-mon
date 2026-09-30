---
summary: "Records the deterministic-test seam debt across Windows hardware telemetry collectors."
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

# Windows Hardware Telemetry Deterministic Test Seam

**Priority:** Medium-high

## Root cause

Windows hardware telemetry still couples collector-level availability, degradation, topology, and partial-success semantics too directly to live hardware and driver calls. Some parsers and local invariants are deterministic, but several source families still lack a narrow replaceable seam between native I/O outcomes and collector policy.

## Evidence

CPU-native telemetry still relies on live processor-topology discovery, thread-affinity binding, and privileged PawnIO register access for end-to-end collector behavior. Storage temperature now has deterministic parsing for structured and SMART data, but device discovery and capability/error classification still originate from live device handles. ACPI thermal-zone, battery-temperature, and Intel D3DKMT temperature success paths remain hardware-dependent; current representative runtime validation cannot cover all of them on one machine. GPU vendor-topology and backend-selection semantics likewise depend on D3DKMT/native runtime outcomes even though their policy should be testable independently.

## Governing constraint

Hardware access remains platform-private, while collector semantics that decide identity, applicability, availability, degradation, channel visibility, backend selection, and partial success must be provable without requiring specific physical hardware whenever those semantics do not inherently depend on hardware behavior.

## Scope discovery

Review Windows telemetry paths that directly perform or consume processor-topology discovery, affinity changes, PawnIO register access, SetupAPI enumeration, storage temperature/SMART IOCTLs, ACPI thermal-zone and battery IOCTLs, D3DKMT adapter queries, and vendor GPU runtime outcomes. Include future Windows hardware sources whose collector policy depends on native-call results.

## Maintenance consequence

Reasonable changes to hardware support, topology handling, capability classification, backend selection, identity, or partial-failure semantics can compile and pass repository validation yet still require representative physical hardware to prove ordinary collector state transitions. This raises verification cost, delays regressions until runtime testing, and makes unsupported/unknown/error distinctions easier to collapse accidentally.

## Repair direction

Introduce the smallest platform-private seams that separate native calls from collector policy. Feed typed native outcomes into pure or deterministic logic for parsing, capability classification, topology state, identity, availability, degradation, backend selection, and snapshot projection. Keep hardware algorithms and source-family semantics local; do not introduce a generic hardware abstraction framework. Retain representative real-hardware tests only for driver/firmware compatibility and ABI behavior that cannot be simulated meaningfully.

## Exit criteria

- Windows CPU collector-level success, unavailable, invalid-channel, topology, affinity, and partial-success states are deterministically testable without live hardware.
- Storage temperature capability/error classification and snapshot projection are deterministically testable from injected structured/SMART outcomes.
- ACPI thermal-zone and battery collectors can deterministically cover valid data, unsupported capability, empty/invalid data, and hard failure without matching physical devices.
- GPU topology can deterministically cover present, absent, and unknown vendors plus backend load/query outcomes without depending on the host GPU set.
- Product and diagnostic projections can be verified without inspecting display labels as program semantics.
- Representative hardware validation remains compatibility evidence, not the primary proof of ordinary collector state transitions.
