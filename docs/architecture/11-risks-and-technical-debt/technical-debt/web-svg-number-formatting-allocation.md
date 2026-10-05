---
summary: "Records avoidable temporary allocation in the Web SVG numeric formatting hot path."
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
---

# Web SVG Number Formatting Allocation

**Priority:** Low-medium

## Root cause

The Web SVG renderer formats each floating-point coordinate through an owned intermediate string before appending the result to the already-existing SVG output buffer. The formatting API therefore imposes allocation and copy work that is not inherent to SVG serialization.

## Primary cost dimension

Runtime CPU and allocation overhead while the browser dashboard has an active subscriber.

## Current cost

The cost repeats for every numeric coordinate emitted while rendering graph points and other Scene geometry. During a production-path profile with an active Web client, the HTTP thread accounted for roughly 14% of sampled process cycles, and floating-point formatting plus `cclover_web_ui::render_points` appeared directly in the sampled hot path. The full HTTP share is not attributable to this mechanism, but the profile establishes that numeric formatting is material enough to be visible in the steady-state renderer workload.

## Evidence

The sampled workload ran the interactive application with desktop and HTTP enabled and one established client connected to the Web endpoint. `perf record` attributed samples in the HTTP thread to Rust floating-point formatting internals and `cclover_web_ui::render_points`. Source inspection during the same investigation showed that each numeric value is converted into a temporary owned string, trimmed, and then copied into the final SVG string.

## Cost mechanism

The renderer already owns the final mutable output buffer, but the numeric formatting boundary returns an owned `String`. Each coordinate therefore pays temporary allocation/ownership work and an additional append/copy step. Graphs multiply that cost by point count and by the one-second dashboard update cadence while subscribers are active.

## Reachable better state

Write the normalized numeric representation directly into the existing output buffer through a small formatter helper, preserving the current three-decimal maximum, trailing-zero trimming, and exact SVG output semantics without introducing a new serialization subsystem.

## Governing constraint

Hot-path Web Scene serialization should not allocate an intermediate owned string solely to transfer a numeric token into an output buffer that already exists, unless measurement shows that the simpler owned representation is cheaper overall.

## Scope discovery

Inspect numeric serialization in `cclover-web-ui`, including graph point lists, rectangle geometry, stroke widths, radii, and any other paths that call the same numeric formatter. Keep presentation formatting and public API JSON serialization out of scope unless independent evidence shows the same governing cause there.

## Repair direction

Replace the owned-return numeric helper with direct output-buffer writing, or an equivalently allocation-free local mechanism. Preserve exact output formatting and escaping behavior. Do not introduce an SVG DOM, generalized serializer framework, persistent coordinate cache, or cross-frame state for this repair.

## Exit criteria

- All current Web SVG output tests remain byte-for-byte equivalent for representative integer, fractional, negative, and trailing-zero cases.
- Numeric Scene serialization no longer creates one owned intermediate string per emitted coordinate in the normal path.
- A production-path profile with an active Web subscriber no longer shows the removed allocation mechanism as a material numeric-formatting hotspot, or measurement demonstrates that the proposed repair is not beneficial and the debt is re-evaluated.
- No new cross-frame cache or duplicated formatting authority is introduced.
