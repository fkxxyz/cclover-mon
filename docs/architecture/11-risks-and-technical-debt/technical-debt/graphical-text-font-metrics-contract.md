---
summary: "Tracks the remaining real-Windows acceptance evidence for the graphical bounded-text font conformance contract."
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
    - ui
---

# Graphical Text Font Metrics Contract

**Priority:** Low-medium

## Root cause

The bounded-text conformance mechanism is implemented across shared Scene semantics, Web, Linux Cairo, and Windows GDI, but the new GDI production-font validation test has not yet executed on a real Windows host for this change. Cross-compilation proves the Windows-only test builds and links; it cannot prove the concrete Windows font realization and fallback behavior that the contract exists to validate.

## Optimization dimension

Primary: test and verification efficiency. Secondary: portability and architecture assurance.

## Current cost

The implementation cannot yet be considered fully accepted across the discovered renderer scope. Until real-Windows execution succeeds, maintainers must carry uncertainty that an actual GDI font realization or fallback differs from the cross-built assumptions, and closing the debt would rely on an unexecuted verification path.

## Evidence

Linux Cairo production-font validation passes locally and the real Chromium browser smoke validates Web text realization. The Windows GDI production-font test is part of the `windows-native` validation profile and compiles and links for both supported Windows targets under cargo-xwin. The current Linux worktree is not Windows-path-mappable through the local host bridge, so that profile has not been executed against this change on a real Windows host.

## Reachable better state

Execute the existing `windows-native` profile on a real Windows host, either through a working repository host bridge or the existing Windows CI runner, and observe the production GDI font-realization test pass. No additional typography architecture or renderer-specific layout logic is required.

## Governing constraint

Every graphical renderer must have executed evidence that its concrete production font realization rejects must-fit overflow while accepting the production bounded-text Scene, without feeding font measurement back into shared geometry.

## Scope discovery

The implementation scope is already discovered: Web SVG/browser, Linux Cairo, and Windows GDI. Before closing this record, confirm that no additional graphical renderer has been added and that the `windows-native` profile still executes the GDI production-font validation test on a real Windows host.

## Repair direction

Run the existing real-Windows validation path; do not add another test framework, bundled font, renderer-specific geometry, or duplicate acceptance harness. If the GDI test fails, treat the failure as new evidence and repair the concrete font-realization path before reassessing closure.

## Exit criteria

- The current change executes `windows-native` on a real Windows host.
- The GDI production-font test accepts the production Scene and rejects the deliberate overflow fixture.
- Web and Linux renderer validation remain green.
- Shared Scene geometry remains the only dashboard layout authority.
- After rechecking renderer scope, remove this record and its index entry.
