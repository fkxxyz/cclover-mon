---
summary: "Records the decision to share one Iced desktop frontend while adapting Linux window integration to Wayland layer-shell or X11."
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

# ADR 002: Shared Iced UI with Native Desktop Integration

## Decision

Use Iced for the shared Rust desktop frontend. Desktop integration selects only the native window host; it does not fork dashboard drawing or presentation logic.

On Linux:

- Wayland sessions use `iced_layershell` and the layer-shell protocol.
- X11 sessions use the normal Iced/winit X11 window path plus X11/EWMH window semantics where Iced does not expose the required desktop-panel behavior.
- Runtime selection prefers Wayland when `WAYLAND_DISPLAY` is present and otherwise uses X11 when `DISPLAY` is present.

On Windows, run the same Iced frontend through its normal native window path, with Win32-specific window behavior kept behind the platform desktop-integration boundary.

Wayland preserves the inherited top-right placement, bottom layer, and zero exclusive zone. X11 should reproduce the same user-visible intent as closely as EWMH/window-manager semantics permit: fixed top-right placement, no decorations, no taskbar/pager entry, no focus stealing, transparent background, and desktop-like stacking.

## Rationale

Iced keeps desktop rendering in Rust, preserves one shared cross-platform desktop frontend, and consumes typed in-process presentation data. Renderer-neutral presentation semantics remain outside Iced so another frontend, such as a terminal UI, does not need to depend on Iced widgets or pixel layout.

Wayland layer-shell and X11/EWMH solve window-management semantics, not drawing. Keeping those choices outside the shared Iced view prevents protocol-specific branches from spreading through widgets, graphs, layout, or presentation.

Both dependencies use permissive licensing and remain owned by Cargo in the single executable build.

## Consequences

- Iced desktop drawing code is shared across Linux Wayland, Linux X11, and Windows; native desktop integration owns only window hosting and window-manager semantics.
- Linux runtime protocol selection belongs to the platform desktop-integration boundary rather than `main.rs` or the UI.
- Sampling remains independent from rendering; renderer-neutral presentation is derived from completed shared `MonitorState` values before Iced rendering.
- Iced-specific layout and widgets are not contracts for non-desktop frontends.
- `iced_layershell` 0.19.1 currently requires `winit-core` and `winit-common` 0.31.0-beta.2 for compatibility. Those versions remain pinned until the integration dependency supports a newer compatible release.
- Pure visual changes currently require rebuilding the Rust application; reloadable UI resources are not part of this implementation.
