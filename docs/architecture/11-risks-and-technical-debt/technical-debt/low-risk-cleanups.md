---
summary: "Collects deferred low-risk structural cleanups that are cheaper to fix than to manage as separate architecture debts."
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

# Low-Risk Cleanup Queue

## Status

Active — P3.

## Purpose

These items are intentionally deferred, but each is local, mechanically verifiable, and does not justify a separate root-cause debt document. When touched, fix the item directly and remove it from this queue. Items that reveal a broader semantic/design choice must be promoted to their own debt before larger refactoring.

## Queue

- Replace `ui::metric_card` / `small_graph_card` argument explosion and `#[allow(clippy::too_many_arguments)]` with small semantic parameter/config structures. This is local API cleanup; do not build a generic widget framework.
- Consolidate duplicated desktop placement margin/constants while the broader [desktop hosting boundary](desktop-hosting-boundary.md) remains active. Removing the duplicate constant alone does not resolve that root debt.
- Rename shared CPU counter fields such as `total_jiffies` / `idle_jiffies` to platform-neutral semantics if the core contract only requires monotonic comparable time units. If unit semantics require a larger cross-platform contract, promote that issue instead of performing a cosmetic rename.
- Rename ambiguous projection types such as `ProcessCpu`, `ProcessMemory`, or `DirectionHistory` when a clearer domain name can be applied mechanically without changing semantics.
- Split metric-specific derivation bodies out of the large core `derive` function into focused helpers for local readability/testability. This does not by itself resolve [metric extension coupling](metric-extension-coupling.md).
- Split `configure_x11_window` into named protocol-policy steps such as input-shape, WM-state, placement, and client-message helpers where doing so improves reviewability. This does not by itself resolve [desktop hosting boundary](desktop-hosting-boundary.md).
- Add a baseline automated validation command/workflow for `fmt`, tests, clippy, and architecture checks where the environment supports them. Keep [architecture enforcement](architecture-enforcement.md) active until dependency-direction and applicable native/build validation are also mechanically protected.

## Exit criteria

The queue is empty because each item was either directly fixed or promoted to a root-cause debt after evidence showed that a local mechanical change was insufficient.
