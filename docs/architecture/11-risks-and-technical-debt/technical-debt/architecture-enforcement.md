---
summary: "Tracks architecture and quality rules that are documented but not mechanically enforced."
viewpoint: assurance
concerns:
  - maintainability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
---

# Architecture Enforcement

## Status

Active — P1; static dependency direction partially mitigated.

## Problem

Several important architecture and validation rules still exist primarily as prose and developer workflow rather than executable constraints. Static dependency direction now has a dedicated fast gate, but the wider required validation set and cross-layer behavioral contracts are not automatically enforced.

## Evidence

`bun archgate.ts` now scans Rust source dependencies and fails on forbidden top-level edges among `core`, `platform`, `presentation`, and `ui`; its focused Bun tests cover allowed/forbidden edges, braced imports, relative `super` paths, comment/string filtering, the live repository, and a violating fixture. Targeted source/contract tests also protect selected native-desktop invariants such as keeping Iced out of native dependencies and retaining required Win32 panel policy primitives. Repository validation commands are still documented workflow rather than CI-enforced automation, and important native runtime behavior remains expensive to establish mechanically: Wayland output scaling, Windows pointer passthrough, desktop/shell Z-order and repaint behavior, Explorer/tray restart recovery, and cross-renderer text/layout parity can compile and satisfy structural tests while still failing on a real compositor or shell.

## Maintenance impact

As contributors, platforms, native integrations, and frontends increase, architecture drift can compile successfully and survive until review or runtime testing. Manual quality gates are also easiest to skip on seemingly small changes.

## Governing constraint

Important architecture invariants and repeatable validation steps should fail mechanically wherever the cost of enforcement is reasonable. Documentation remains authoritative for intent, while automation protects the invariant from accidental erosion.

## Resolution direction

Keep `archgate.ts` narrowly focused and millisecond-scale; do not add compilation, formatting, Clippy, or runtime work to it. Add further targeted mechanical checks only when an architecture invariant has a similarly cheap, deterministic representation. Full repository validation automation may be added later when the project adopts CI. Prefer targeted checks over splitting crates solely for enforcement, and add contract-level tests where a documented boundary has meaningful behavior not covered by local unit tests. For behavior whose correctness depends on a real desktop environment, keep a small representative runtime smoke matrix rather than pretending source inspection proves protocol semantics; prioritize checks for output scaling, input passthrough, shell stacking/repaint, tray restart recovery, and graphical layout parity because those have already produced real regressions during the native-renderer migration.

## Exit criteria

Critical dependency-direction violations remain mechanically detectable with millisecond-scale latency. The debt can close when the remaining required repository validation and important cross-layer behavioral contracts have proportionate mechanical enforcement appropriate to the project's contribution workflow.
