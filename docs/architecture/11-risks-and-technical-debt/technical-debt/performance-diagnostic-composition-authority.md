---
summary: "Records that production and performance collector orchestration can still drift because production-equivalent composition is maintained in more than one platform path."
viewpoint: assurance
concerns:
  - performance
  - maintainability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - platform
    - whole-system
---

# Performance Diagnostic Composition Authority

**Priority:** Low-medium

## Root cause

Production sampling composition and `perf collector` composition remain separate platform orchestration paths. The compiler can require every `ProbeKind` to be handled, but it cannot prove that the performance path prepares the same production context, lifecycle state, and product projection as the corresponding production call when those semantics evolve.

## Primary cost dimension

Maintenance and performance-diagnosis reliability.

## Current cost

Maintainers must compare production and performance call structure whenever a collector gains a new prerequisite context, lifecycle step, projection rule, topology dependency, or shared observation. A change can compile and pass local tests while leaving `perf collector` semantically stale, causing later performance work to measure a different workload and spend time reconciling contradictory results.

## Evidence

Two distinct drifts were discovered in the same performance-fidelity repair. Linux disk/network attribution performance workloads omitted the complete active-process context that production uses to retire stale eBPF map entries. Windows temperature/fan performance workloads inherited probe-specific diagnostic projections even though production sampling uses coordinated hardware collection and product projections. Both paths invoked real collector implementations; composition, not collector code reuse, was the missing authority.

The current repair fixes those known instances and makes platform performance dispatch exhaustive, but production and performance orchestration still remain separately maintained representations of the production-equivalence rule.

## Cost mechanism

One conceptual rule — the minimum production composition required for a faithful isolated workload — is encoded independently in production sampling and performance dispatch. When production composition changes, no structural mechanism necessarily updates the performance path. Human synchronization is therefore part of the maintenance contract, and drift can survive compilation because both representations are individually valid.

## Reachable better state

Where repeated pressure exists, move only the semantics that determine production equivalence into small platform-private composition helpers or typed preparation results consumed by both production and performance paths. Keep probe orchestration separate when its diagnostic projections intentionally differ. Do not create a generic dependency graph, benchmark DSL, or cross-platform context framework unless future repetition proves that such abstraction is cheaper than local shared authorities.

## Governing constraint

Any context, lifecycle step, shared observation, identity preparation, or product projection whose semantics determine the selected collector's production cost should have one platform authority consumed by both production sampling and production-faithful performance diagnostics.

## Scope discovery

For each supported platform, compare every `perf collector` branch with the production sampling path for the same metric. Identify duplicated decisions about prerequisite process/device snapshots, shared hardware batches, product versus diagnostic projections, topology refresh, failure/backoff state, identity normalization, and bounded-state retirement. Distinguish intentional probe-only differences from production-equivalence rules that should not be maintained twice.

## Repair direction

Refactor only duplicated production-semantic decisions that already create synchronization pressure. Prefer small platform-private helpers near the owning backend and existing collector APIs. Preserve exhaustive dispatch as a change trigger, but do not treat exhaustiveness alone as proof of semantic equivalence.

## Exit criteria

- Every context-dependent `perf collector` workload derives its production-equivalence preparation from the same platform authority used by normal sampling, or has an explicit documented reason why faithful isolation is unavailable.
- Probe-only diagnostic projections remain independent where they intentionally differ from production semantics.
- Adding or changing a production prerequisite for a collector cannot silently leave an independently maintained performance equivalent behind without deterministic or compile-time evidence failing.
- No generic dependency graph, synthetic benchmark implementation, or second collector hierarchy is introduced solely to satisfy this debt.
- A representative production composition change can be made at one semantic control point without manually synchronizing equivalent lifecycle or projection policy in `collect_for_perf`.
