---
summary: "Records the deterministic-test seam debt in Windows CPU hardware telemetry."
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

# Windows CPU Telemetry Deterministic Test Seam

**Priority:** Medium-high

## Root cause

Windows CPU hardware telemetry couples collector behavior directly to live processor-topology discovery, thread-affinity binding, and privileged PawnIO MSR access. Pure parsing and selected topology invariants are testable, but collector-level success, partial-success, and failure behavior still lacks a narrow deterministic hardware seam.

## Evidence

Recent Intel temperature work required privileged Windows runtime validation to distinguish topology parsing, affinity binding, and per-core DTS behavior. The topology parser now has deterministic coverage, and topology discovery failure is explicitly tested not to disable the independent package capability, but end-to-end collector behavior across affinity and MSR outcomes still depends on live hardware.

## Governing constraint

Hardware access remains platform-private, while collector semantics that decide availability, degradation, channel visibility, and partial success must be provable without requiring specific physical hardware whenever those semantics do not inherently depend on hardware behavior.

## Scope discovery

Review Windows CPU-native telemetry paths that directly perform or consume processor-topology discovery, thread-affinity changes, PawnIO MSR/SMN/PCI reads, and their availability/degradation decisions. Include Intel and AMD collectors and any future CPU-native source that makes collector semantics depend on live hardware calls.

## Maintenance consequence

Reasonable changes to CPU-family support, topology handling, affinity policy, register reads, or partial-failure semantics can compile and pass repository validation yet still require privileged representative hardware to establish basic collector correctness. That raises verification cost and increases the chance of regressions reaching runtime validation late.

## Repair direction

Introduce the smallest platform-private replaceable seam needed to supply topology, affinity, and register-access outcomes to CPU collectors. Keep hardware algorithms and source-family semantics local; avoid a generic hardware abstraction framework. Use deterministic contract tests for availability, degradation, partial success, and product-versus-diagnostic projection, while retaining representative real-hardware tests for behavior that cannot be simulated meaningfully.

## Exit criteria

- Intel and AMD collector-level tests can deterministically cover successful reads, unavailable reads, invalid channels, and partial success without live hardware.
- Topology and affinity failures can be injected independently of register-read results.
- Package/main CPU observations remain available when only finer-grained topology or channels fail.
- Product projection and diagnostic projection are covered without inspecting display labels or opaque sensor IDs.
- Representative hardware validation remains only for hardware/driver compatibility evidence, not for proving ordinary collector state transitions.
