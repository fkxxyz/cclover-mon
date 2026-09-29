---
summary: "Records duplicated renderer invalidation authority across scene classification, hashing, bounds, damage tracking, and drawing."
viewpoint: assurance
concerns:
  - maintainability
  - performance
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
---

# Renderer Invalidation Authority

## Priority

Medium.

## Root cause

Incremental native rendering currently depends on several separately maintained descriptions of the same primitive semantics. Static/dynamic classification, visual hashing, command bounds, damage propagation, and draw behavior each encode overlapping facts about when a primitive changes and which pixels that change can affect.

## Evidence

The Linux native renderer has separate logic for static-content hashing, dynamic-command hashing, command bounds, dirty-rectangle accumulation, and primitive drawing. Shared scene lowering separately marks primitives as static or dynamic and supplies geometry and flags consumed by those paths.

The current steady-state fast path is mechanically guarded against obvious regressions such as rebuilding the same scene twice per frame, recreating stable Wayland buffers, rebuilding an unchanged static layer, or forcing dynamic-only changes through full-surface drawing. Those tests reduce verification cost but do not remove the duplicated semantic authority.

## Governing constraint

For each native scene primitive, the facts that determine visual identity, affected bounds, cacheability, and drawing semantics must have one authoritative definition or be mechanically derived from one authoritative description. Adding or changing a primitive must not require maintainers to remember a hidden set of synchronized invalidation rules.

## Scope discovery

When resolving this debt, inspect all native scene primitive definitions and every consumer that:

- classifies content as static or dynamic;
- computes visual hashes or equality for cache invalidation;
- computes primitive or damage bounds;
- merges or applies dirty regions;
- draws the primitive on Linux or other native renderers;
- maps shared scene data into the native ABI.

Repeat this discovery against the current codebase before closure; listed functions are evidence, not the complete repair scope.

## Maintenance consequence

A new visual field, primitive kind, alignment rule, clipping rule, or geometry change can remain correct in one path while being omitted from another. Failure may appear as stale pixels, unnecessary full redraws, incorrect cache reuse, or a silent CPU regression. Because many such failures are runtime-visual or workload-dependent, omissions are more expensive to diagnose than ordinary compile-time breakage.

## Repair direction

Move primitive visual identity, damage bounds, and cacheability toward one declarative or otherwise single-authority representation from which renderer-specific invalidation behavior can be derived. Keep platform drawing APIs private to their renderer; do not introduce a cross-platform rendering framework merely to remove duplicated invalidation logic.

## Exit criteria

This debt is resolved when scope discovery shows that a primitive-semantic change has one authoritative edit point for invalidation-relevant facts, all renderer cache/damage consumers derive from that authority, and regression tests prove the steady-state incremental path without requiring manual synchronization across separate hash/bounds/classification implementations.
