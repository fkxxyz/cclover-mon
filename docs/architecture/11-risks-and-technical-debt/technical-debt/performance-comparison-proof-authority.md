---
summary: "Records that performance changes lack a repository-owned, repeatable baseline-versus-candidate comparison proof."
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
    - whole-system
    - core
    - platform
---

# Performance Comparison Proof Authority

**Priority:** Low-medium

## Root cause

The repository owns production-path performance workloads such as `perf headless` and `perf collector <name>`, but it does not own the comparison procedure that turns those workloads into repeatable baseline-versus-candidate evidence. Maintainers must currently assemble run ordering, repetition, environment observations, CPU-time aggregation, and noise interpretation manually.

## Primary cost dimension

Performance validation and maintenance effort.

## Current cost

Small performance changes require repeated manual experiment setup and interpretation before maintainers can tell whether a measured delta is real. Short runs can produce conflicting results, so maintainers may need to rebuild a baseline binary, alternate baseline and candidate executions, extend run duration, compare process counts, and manually aggregate user/system CPU time. This increases diagnosis time and creates avoidable risk of accepting noise as an optimization or rejecting a real improvement.

## Evidence

During Linux process-collector optimization work, the production collector workload was sufficient to exercise the intended code path, but a single 20-sample comparison was too noisy to establish whether userspace fast-path changes helped. The investigation then required multiple repeated runs followed by same-machine alternating baseline/candidate executions and a longer 30-sample comparison. The longer comparison showed approximately 165 ms CPU over 29 seconds for the baseline and 166.5 ms for the candidate, demonstrating that the apparent earlier differences were noise and that the more complex implementation should be discarded.

The useful result came from the comparison discipline, not from a repository-owned proof mechanism; the same orchestration would have to be reconstructed for future small performance changes.

During later Wayland damage-aware Cairo culling work, two short baseline/candidate rounds again disagreed in isolation. The first steady-state round measured approximately 8.607 ms/frame for the baseline versus 5.844 ms/frame for the candidate, while the second measured approximately 4.470 ms/frame for the baseline versus 5.026 ms/frame for the candidate. Only after alternating runs and combining 18 steady frames per side did the aggregate indicate approximately 6.768 ms/frame versus 5.480 ms/frame, about a 19% reduction. Producing that evidence required transient profile logs plus ad hoc parsing and aggregation rather than a repository-owned comparison command, reinforcing that a single favorable run is not sufficient proof for small renderer optimizations.

## Cost mechanism

The repository defines what production work to measure but not how to compare two revisions under controlled, sufficiently repeated conditions. Measurement procedure therefore lives in transient shell commands and maintainer judgment. Each performance investigation recreates parts of the same experimental protocol, and different investigations can use different run lengths, ordering, aggregation, or environmental checks, reducing comparability and confidence.

## Reachable better state

Keep the existing production-path workload commands as the workload authority, and add the smallest repository-owned comparison layer that can run an explicit baseline and candidate repeatedly under the same selected workload, alternate execution order, record relevant workload context, summarize user/system/total CPU time and dispersion, and report the relative delta without inventing a second benchmark implementation.

The comparison layer should support an inconclusive result when observed differences are within run-to-run noise. It should not impose a universal hard performance threshold across heterogeneous machines.

## Governing constraint

Performance claims used to accept or reject an optimization must be reproducible from repository-owned production workloads and a repository-owned comparison procedure, with enough context and repeated measurement to distinguish material change from ordinary run-to-run noise.

## Scope discovery

Review performance workflows that compare revisions rather than merely attribute current cost. Include `perf headless`, every `perf collector` workload, documented performance-validation procedures, release or regression checks that make comparative claims, and any ad hoc scripts that duplicate baseline/candidate orchestration. Keep workload-fidelity concerns governed by [Performance Diagnostic Workloads](../../13-performance-view/diagnostic-workloads.md); this debt governs comparison and proof once a trustworthy workload has been selected.

## Repair direction

Add a thin comparison harness around the existing diagnostic workload authority. Prefer explicit executable paths or revisions supplied by the caller rather than hidden rebuild policy. Capture enough metadata to interpret a run, such as workload, sample/duration settings, run count, and relevant workload cardinality when available. Alternate baseline and candidate order to reduce temporal bias, report robust summaries rather than a single run, and surface inconclusive/noisy results instead of forcing a winner.

Do not create synthetic collector benchmarks, copy collector logic into the harness, or introduce a large statistical framework. The harness should standardize experiment orchestration and reporting, not become a second performance subsystem.

## Exit criteria

- A repository-owned comparison command can run the same supported production-path diagnostic workload against an explicit baseline and candidate.
- Comparison executes multiple runs with order balancing or alternation rather than relying on one baseline followed by one candidate run.
- Output reports user, system, and total CPU-time summaries plus enough spread/context to identify noisy or non-comparable results.
- The mechanism can report an inconclusive result instead of interpreting every numeric difference as meaningful.
- Workload definition remains owned by the existing `perf headless` / `perf collector` implementation; no parallel benchmark implementation is introduced.
- Documentation directs comparative performance claims through the shared comparison mechanism rather than requiring maintainers to reconstruct shell-level A/B procedures.
- A representative small-change experiment can be repeated without bespoke shell orchestration and yields enough evidence to accept, reject, or declare the optimization inconclusive.
