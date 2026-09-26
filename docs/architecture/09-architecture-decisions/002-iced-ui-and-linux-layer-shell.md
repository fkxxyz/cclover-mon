---
summary: "Records the decision to use Iced for shared presentation and iced_layershell for Linux layer-shell placement."
viewpoint: decision
concerns:
  - performance
  - portability
  - maintainability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
---

# ADR 002: Iced UI with Linux Layer Shell

## Decision

Use Iced for the shared Rust UI. On Linux, use `iced_layershell` to place the same UI through the Wayland layer-shell protocol. On Windows, run the shared Iced UI as a normal native application window.

Linux layer-shell configuration preserves the product behavior inherited from the Quickshell monitor: top-right placement, bottom layer, and zero exclusive zone.

## Rationale

Iced keeps presentation in Rust, preserves one shared cross-platform UI, and exchanges typed in-process state directly with the core. `iced_layershell` adds the Linux-specific desktop placement without moving collection or presentation semantics into the platform backend.

Both dependencies use permissive licensing and remain owned by Cargo in the single executable build.

## Consequences

- UI code is shared across Linux and Windows while Linux window placement remains platform-specific.
- Sampling remains independent from rendering; the UI receives completed shared `MonitorState` values.
- `iced_layershell` 0.19.1 currently requires `winit-core` and `winit-common` 0.31.0-beta.2 for compatibility. Those versions remain pinned until the integration dependency supports a newer compatible release.
- Pure visual changes currently require rebuilding the Rust application; reloadable UI resources are not part of this implementation.
