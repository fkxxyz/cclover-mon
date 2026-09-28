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

Resolved.

## Problem

The source architecture separates core, presentation, platform, desktop UI, terminal UI, and Web responsibilities, but the Cargo compilation boundary does not follow those change axes.

Most native code remains in one primary crate, and unrelated frontend stacks participate in the same development and validation graph. Logically local changes therefore frequently invalidate or require compilation of substantially more code than their architectural scope requires.

## Evidence

- Core now lives in `cclover-core`, renderer-neutral presentation in `cclover-presentation`, terminal rendering/lifecycle in `cclover-tui`, shared Iced renderer/layout in `cclover-desktop-ui`, and native desktop application/hosting in `cclover-desktop`; platform collectors, `src/web.rs`, and runtime composition remain in the primary application crate.
- `cclover-tui` owns `ratatui` and `crossterm`, so TUI-only validation no longer compiles Iced/WGPU. `cclover-desktop-ui` owns desktop renderer/layout code. `cclover-desktop` owns native Iced application composition plus Linux layershell/tray/X11 hosting; the primary application crate no longer directly declares native Iced, WGPU, layershell, tray, or X11 desktop-host dependencies.
- Recent development frequently changes shared core types and architecture boundaries, so recompilation cost repeatedly appears during normal refactoring rather than only during rare clean builds.
- Full validation spends materially more time compiling than executing the unit tests themselves.
- Core-only changes can now be validated with `cargo test -p cclover-core`; that package currently has no external dependencies. Presentation-only changes can be validated with `cargo test -p cclover-presentation`, whose dependency tree contains only `cclover-core`. TUI-only changes can be validated with `cargo test -p cclover-tui`, whose dependency tree contains presentation/core plus Ratatui/Crossterm but no Iced/WGPU. These workflows avoid unrelated frontend and platform stacks.

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

## Resolution

The workspace now uses package boundaries aligned with the stable change axes that previously caused the largest invalidation radius:

- `cclover-core` owns the platform-neutral metric model, sampling, derivation, and history;
- `cclover-presentation` owns renderer-neutral dashboard semantics;
- `cclover-tui` owns Ratatui/Crossterm terminal rendering;
- `cclover-desktop-ui` owns shared Iced renderer/layout code;
- `cclover-desktop` owns native desktop application composition and Linux/Windows desktop hosting;
- the root `cclover-mon` package remains the application/runtime/platform/Web composition boundary.

Optional heavy build capabilities are also explicit: default/full builds enable `http` and `ebpf-io`, while `--no-default-features` provides a supported minimal native validation path without the WASM/bindgen or eBPF/libbpf toolchains.

Measured warm feedback paths after the split are approximately 0.15 s for `cclover-core`, 0.15 s for `cclover-presentation`, 0.20 s for `cclover-tui`, 0.40 s for `cclover-desktop-ui`, 0.31 s for `cclover-desktop`, and 0.25 s for a root minimal check in the measured development environment. These workflows no longer compile unrelated heavyweight frontend stacks.

Further package splitting of runtime, platform collectors, or Web transport was evaluated and rejected for now because the remaining compilation radius is already small and those responsibilities still form the application composition boundary. Additional packages would add API/package management cost without a demonstrated feedback-loop benefit.

## Exit criteria

This debt closes because:

- core-only changes can be type-checked and unit-tested without compiling Iced, WGPU, Ratatui, or other unrelated frontend stacks;
- presentation-only changes can be validated without compiling unrelated renderer or platform implementations unless their public contract is affected;
- frontend-specific changes do not force recompilation of unrelated frontends;
- full application builds still produce the intended single executable per target;
- architecture dependency direction remains mechanically enforced across any new package boundaries;
- representative incremental edit-and-test workflows demonstrate a materially smaller compilation and validation radius than the pre-split structure;
- release runtime behavior and performance-sensitive sampling paths remain unchanged by the package split; full release validation still succeeds, and no LTO change is justified by current evidence.

Scope discovery was repeated against the final Cargo graph before closure. The intended release deployment remains one process and one application executable per target, while architecture direction remains mechanically enforced across package boundaries by `archgate`.
