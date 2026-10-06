---
summary: "Records the missing repository-owned production-faithful workload for active Web rendering performance investigations."
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
    - ui
    - whole-system
---

# Active Web Performance Workload Authority

**Priority:** Low-medium

## Root cause

The repository owns production-faithful headless and collector performance workloads, but it does not own an equivalent workload for the active browser-rendering path. Measuring Web rendering therefore requires maintainers to reconstruct HTTP startup, readiness, an established SSE subscriber, production sampling cadence, profiler attachment, and process cleanup outside the performance-workload authority.

## Primary cost dimension

Performance-diagnosis and verification efficiency.

## Current cost

Each active-Web performance investigation requires manual orchestration before meaningful attribution can begin. Maintainers must independently decide how to start the server, wait for readiness, keep `/events` subscribed so dashboard rendering is actually active, attach the profiler to the correct process, hold the workload for a comparable interval, and clean up both server and client.

This work has already repeated across Web rendering investigations. It also creates measurement risk: accidentally profiling an idle HTTP server, using different subscriber timing between baseline and candidate, or leaving lifecycle differences uncontrolled can produce results that look comparable while measuring different workloads.

## Evidence

The Web SVG numeric-formatting investigation required a hand-composed production run consisting of a release server, `/readyz` polling, a persistent `/events` SSE client, `perf record` attachment, fixed run duration, and explicit teardown. The same orchestration was reconstructed again to compare the baseline and candidate implementations and establish that the former `cclover_web_ui::number` allocation chain had disappeared.

The existing `perf headless` and `perf collector` workloads intentionally exclude UI and transport work, so they cannot prove active Web renderer cost. `perf-compare.ts` correctly reuses those existing workload authorities and therefore cannot represent active-Web rendering without a corresponding production workload.

## Cost mechanism

One conceptual workload — normal production sampling with an active Web subscriber — has no repository-owned execution authority. Its lifecycle and equivalence conditions therefore live in ad hoc shell orchestration and maintainer memory. Every later Web serialization, Scene rendering, SSE publication, or transport performance investigation can recreate the same setup and the same opportunities for drift.

## Reachable better state

Add the smallest repository-owned active-Web performance workload that exercises the existing production runtime, HTTP state publication, Scene construction, SVG rendering, and established SSE subscription under normal sampling cadence. The workload should own only the lifecycle needed to make that production path repeatable and externally profileable.

It should remain a workload authority, not become a benchmark framework, synthetic renderer, browser automation suite, or second HTTP implementation.

## Governing constraint

Any repository-supported performance comparison of active Web rendering must derive its workload lifecycle from one production-faithful authority rather than from independently reconstructed server/subscriber orchestration.

## Scope discovery

Inspect the existing performance CLI, `perf-compare.ts`, HTTP server lifecycle, readiness semantics, latest-state publication, dashboard-render demand gating, SSE subscription lifecycle, and shutdown handling. Identify the minimum production path required to guarantee that Web rendering is continuously active while an external profiler measures the real process.

Keep browser font/layout conformance, public API projection, synthetic rendering benchmarks, and collector-specific diagnostics out of scope unless they are independently required to preserve the measured active-Web production semantics.

## Repair direction

Extend the existing performance workload authority with one narrow active-Web mode, or an equivalently small repository-owned launcher, that starts the production HTTP path, establishes a real SSE subscriber, waits until the rendering path is active, preserves normal sampling cadence, and terminates deterministically after fixed work.

Reuse existing production implementations and comparison machinery. Do not add a benchmark DSL, duplicate renderer implementation, synthetic Scene generator, generalized process supervisor, or browser requirement solely for this debt.

## Exit criteria

- One repository-owned workload can deterministically exercise the production active-Web rendering path with an established SSE subscriber.
- The workload preserves normal production sampling and HTTP/SSE rendering semantics rather than substituting synthetic rendering work.
- Baseline and candidate executions can use the same fixed-work definition without manually reconstructing readiness, subscription, timing, or cleanup policy.
- External profilers can attach to or wrap the workload without changing what production work is performed.
- Existing `perf headless` and `perf collector` authorities remain focused on their current non-Web responsibilities.
- No benchmark framework, duplicate renderer, or browser automation dependency is introduced solely to satisfy this debt.
