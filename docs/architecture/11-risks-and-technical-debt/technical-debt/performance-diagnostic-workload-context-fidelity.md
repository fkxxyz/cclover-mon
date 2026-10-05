---
summary: "Records that production-faithful collector workloads still need real eBPF runtime acceptance before context-fidelity debt can be closed."
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
    - core
    - platform
    - whole-system
---

# Performance Diagnostic Workload Context Fidelity

**Priority:** Medium

## Root cause

`perf collector <name>` historically isolated production collector implementations without a governing contract that preserved every piece of production sampling context affecting lifecycle and steady-state work. The implementation now preserves the discovered context and projection requirements, but the Linux eBPF repair has not yet completed real privileged runtime acceptance, so the original cost-producing mechanism is not yet proven closed end to end.

## Primary cost dimension

Performance diagnosis and optimization effort.

## Current cost

Maintainers still lack final runtime proof that the repaired Linux attribution workloads retire stale eBPF state under the same conditions as normal production sampling. Until that proof exists, a future performance investigation could still rely on a workload whose intended production fidelity is established statically and deterministically but not yet confirmed against the privileged native path that originally exposed the defect.

## Evidence

During Linux CPU investigation, the isolated `network-attribution` workload initially appeared to consume about 2% of one core and looked like a dominant steady-state cost. Inspection showed that isolated attribution collection lacked the complete active-process context used by normal sampling to retire stale eBPF attribution entries. Production-path measurement then showed steady-state network attribution around 0.39 ms per sample and disk attribution around 0.83 ms per sample, materially changing optimization priority.

The current repair shares Linux active-process context between production and performance paths, distinguishes probe orchestration from production-faithful performance orchestration, and has deterministic unit coverage plus Linux and Windows cross-platform validation. The remaining gap is real Linux execution of the repaired eBPF attribution workloads with the required file capabilities.

Later eBPF map-batch A/B work exposed the same boundary from another direction. High-cardinality `disk-attribution` and `network-attribution` runs established a clear CPU reduction from batched map lookup, but those isolated workloads did not provide active-process context and therefore did not execute stale-entry retirement. A first implementation also batched deletion even though the experiment had not isolated or established material deletion cost. The unproven batch-delete path was removed rather than retained on the strength of broader `perf headless` measurements. This is concrete evidence that missing lifecycle context can encourage optimization complexity beyond what the selected workload actually proves.

## Cost mechanism

The original failure mode arose because isolation removed context that bounded native state. The implementation now removes that known divergence, but closing the debt before executing the repaired privileged runtime path would replace a semantic defect with an assurance gap: the repository would claim the mechanism retired without proving the exact native lifecycle behavior that justified the debt.

## Reachable better state

Run the repaired `perf collector disk-attribution` and `perf collector network-attribution` workloads with the required eBPF capabilities under a short-lived-process workload, and confirm stale attribution state converges as it does under production sampling. Keep the deterministic tests and architecture contract as the cheap regression proof after that runtime acceptance establishes the native boundary once.

## Governing constraint

A production-path performance diagnostic may remove unrelated work, but it must not change lifecycle, identity, bounded-state, projection, or other context semantics that materially determine the cost of the work being measured; closure requires proof at the cheapest layer capable of establishing each part of that contract, including real native execution where simulation cannot prove it.

## Scope discovery

Review every `perf collector` workload against its corresponding production invocation and identify inputs or lifecycle state obtained from the coordinated sampling cycle. For the remaining acceptance gap, exercise Linux disk and network attribution with real eBPF maps, complete process snapshots, PID reuse/short-lived-process churn, and repeated samples long enough to observe stale-state retirement.

## Repair direction

Do not redesign the implementation further unless runtime evidence reveals a remaining mismatch. Complete the privileged Linux acceptance on the current repaired path. If the runtime result matches production semantics, delete this debt record and retain the durable performance-diagnostic contract in the performance and platform Views.

## Exit criteria

- Scope discovery covers every currently supported `perf collector` workload.
- Each context-dependent collector receives production-equivalent context or is explicitly excluded from faithful isolated steady-state claims.
- Deterministic tests enforce the repaired Linux active-process-context semantics and platform perf dispatch continues to compile for supported Linux and Windows targets.
- Real Linux `disk-attribution` and `network-attribution` performance workloads execute with eBPF enabled and confirm stale-process retirement under process churn.
- Repeating attribution measurement no longer produces a material priority inversion solely because the diagnostic workload retains state that production would retire.
- Performance documentation states the trustworthy isolation boundary without maintaining a second divergent workload definition.
