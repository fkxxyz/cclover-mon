---
summary: "Records that positional Scene diffing can amplify local dynamic insertions or removals into large conservative damage regions."
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
    - native-bridge
---

# Scene Positional Invalidation Damage Amplification

**Priority:** Low-medium

## Root cause

Dynamic Scene invalidation uses structural primitive equality as its conservative identity and currently pairs old and new dynamic primitives by ordinal position. This keeps the diff simple and correct, but insertion or removal before otherwise unchanged primitives shifts the remaining ordinal alignment and can make an unchanged tail appear changed.

## Primary cost dimension

Runtime rendering efficiency.

## Current cost

Local dynamic-list changes can produce damage much larger than the pixels whose semantic content actually changed. Cairo then restores and rerenders that larger region even though the renderer correctly avoids a full-panel redraw. The cost is paid whenever dynamic collections change cardinality and the shifted tail covers a substantial portion of the dashboard.

## Evidence

The incremental-rendering repair removed the previous unconditional full redraw on dynamic primitive count changes. In a real Wayland production-path run after that repair, 13 steady frames produced no full redraws and average dirty area was approximately 86,754 of 354,120 pixels (24.5%), but the largest observed frame still damaged approximately 290,140 pixels, about 82% of the panel. The current positional policy intentionally treats shifted primitives conservatively, so this large-damage mode remains reachable without a correctness defect.

## Cost mechanism

When a primitive is inserted or removed before a dynamic tail, positional comparison pairs different semantic primitives at each subsequent index. Each mismatch unions old and new bounds into damage, and neighboring damage can merge into a large region. The renderer then performs avoidable raster work proportional to that conservative region rather than only the genuinely changed collection slots.

## Reachable better state

Retain conservative correctness while giving dynamic primitives enough stable identity, or using an equivalently small topology-aware matching rule, that insertion or removal does not invalidate unrelated following primitives solely because their ordinal positions shifted. The better state does not require globally minimal diffing or a general virtual-DOM algorithm.

## Governing constraint

Dynamic insertion or removal must never under-damage changed pixels, but unrelated primitives should not become dirty solely because a preceding primitive changed cardinality when a stable, economical identity can distinguish them.

## Scope discovery

Review the shared Scene invalidation policy and every dashboard region whose dynamic primitive stream can change cardinality, including process and I/O Top-N rows and any future variable-length graphical collection. Keep renderer-specific clipping and buffer policy out of identity decisions; invalidation identity remains a shared Scene concern.

## Repair direction

Do not add stable IDs, keyed diffing, or sequence algorithms until repeated profiling shows that positional amplification remains a material steady-state cost after the current full-redraw fix. If evidence justifies repair, prefer the smallest shared identity mechanism that covers variable-length dynamic collections and preserves existing primitive damage bounds and pixel-equivalence tests. Avoid renderer-specific or I/O-row special cases.

## Exit criteria

- Representative dynamic insertion and removal no longer damage unrelated shifted tails solely due ordinal mismatch.
- Incremental Cairo output remains pixel-equivalent to a fresh full redraw across append, insertion, removal, movement, and overlap cases.
- Static-change and surface-size changes still force the existing conservative full-redraw path.
- Real Wayland profiling under cardinality-changing workload shows materially lower large-damage frequency or area than the positional policy.
- The repair does not introduce a general diff framework whose maintenance cost exceeds the measured rendering benefit.
