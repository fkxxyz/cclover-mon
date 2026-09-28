---
summary: "Tracks excessive Rust compilation and validation invalidation caused by Cargo boundaries that do not align with architectural change axes."
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

# Crate Boundary Compilation Coupling

## Status

Active — P1.

## Problem

The source architecture separates core, presentation, platform, desktop UI, terminal UI, and Web responsibilities, but the Cargo compilation boundary does not follow those change axes.

Most native code remains in one primary crate, and unrelated frontend stacks participate in the same development and validation graph. Logically local changes therefore frequently invalidate or require compilation of substantially more code than their architectural scope requires.

## Evidence

- `src/core`, `src/presentation.rs`, `src/platform`, `src/ui`, `src/tui.rs`, `src/web.rs`, and runtime composition are part of the same primary crate.
- Native dependencies include both the desktop stack (`iced`, WGPU, `iced_layershell`) and terminal stack (`ratatui`, `crossterm`).
- Recent development frequently changes shared core types and architecture boundaries, so recompilation cost repeatedly appears during normal refactoring rather than only during rare clean builds.
- Full validation spends materially more time compiling than executing the unit tests themselves.
- A core-only change cannot currently be validated with an isolated package command such as `cargo test -p cclover-core` because no corresponding Cargo boundary exists.

## Governing constraint

Compilation and validation boundaries should align with stable architectural change axes.

A change confined to core, presentation, or one frontend should be type-checkable and testable without compiling unrelated heavyweight frontend stacks unless the changed public contract actually affects them.

Build-time boundaries must not change the existing single-process, single-executable deployment model. Release optimization must remain able to optimize across package boundaries where measurement shows that it matters.

## Scope discovery

Trace Cargo dependency and validation relationships for:

- core model, derivation, history, and sampling;
- renderer-neutral presentation;
- platform collectors and desktop integration;
- desktop Iced frontend;
- terminal frontend;
- HTTP/Web transport and frontend;
- application/runtime composition;
- build scripts and target-specific dependencies;
- validation commands and package/feature selection.

Identify which dependency edges force desktop, terminal, Web, or platform-specific dependencies into otherwise unrelated edit-test workflows. Repeat this discovery against the current Cargo graph before closing the debt.

## Maintenance impact

The project currently changes architecture and shared domain types frequently. Because compilation invalidation radius is larger than change radius:

- edit-feedback cycles are unnecessarily long;
- unrelated frontend dependencies consume compilation work during core changes;
- frequent validation makes small refactors disproportionately expensive;
- the cost will grow as more frontends, platforms, and optional capabilities are added.

Long feedback cycles also make frequent local verification less attractive, increasing the practical cost of safe refactoring.

## Resolution direction

Introduce the smallest Cargo package or feature boundaries that reflect existing architectural ownership and measurably reduce unrelated compilation.

Likely candidates include isolating stable core and presentation libraries first, then isolating heavyweight frontend or platform dependencies where measurements show useful compile-time separation. Do not split into fine-grained crates merely to reduce file size, and do not treat workspace conversion itself as success.

Measure representative incremental edit-and-test workflows before and after each structural change. Prefer package boundaries when independent compilation and validation are the goal; use Cargo features only when they represent genuine optional build capability.

## Exit criteria

This debt can close when:

- core-only changes can be type-checked and unit-tested without compiling Iced, WGPU, Ratatui, or other unrelated frontend stacks;
- presentation-only changes can be validated without compiling unrelated renderer or platform implementations unless their public contract is affected;
- frontend-specific changes do not force recompilation of unrelated frontends;
- full application builds still produce the intended single executable per target;
- architecture dependency direction remains mechanically enforced across any new package boundaries;
- representative incremental edit-and-test workflows demonstrate a materially smaller compilation and validation radius than the current structure;
- release runtime performance remains acceptable, using LTO or other cross-crate optimization only where measurement justifies it.

Closure requires repeating scope discovery against the current Cargo graph; creating a workspace or splitting one crate is not sufficient by itself.
