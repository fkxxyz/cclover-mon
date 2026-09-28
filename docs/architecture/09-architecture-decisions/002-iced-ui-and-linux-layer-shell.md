---
summary: "Records the decision to share one Iced desktop frontend while keeping native window and system-tray integration behind platform boundaries."
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

Use Iced for the shared Rust panel frontend. Native desktop integration owns both monitor-surface hosting and platform shell integration; browser/WASM hosting owns only Web runtime and transport concerns. Neither responsibility forks dashboard drawing or presentation logic.

On Linux:

- Wayland sessions use `iced_layershell` and the layer-shell protocol.
- X11 sessions use the normal Iced/winit X11 window path plus X11/EWMH window semantics where Iced does not expose the required desktop-panel behavior.
- Runtime selection prefers Wayland when `WAYLAND_DISPLAY` is present and otherwise uses X11 when `DISPLAY` is present.
- X11 and Wayland share one StatusNotifierItem system-tray implementation over the session D-Bus; tray integration is not selected by display protocol.

On Windows, run the same Iced frontend through its normal native window path, with Win32-specific window and future notification-area behavior kept behind the native desktop-integration boundary in `cclover-desktop`.

Wayland preserves the inherited top-right placement, bottom layer, and zero exclusive zone. X11 should reproduce the same user-visible intent as closely as EWMH/window-manager semantics permit: fixed top-right placement, no decorations, no taskbar/pager entry, no focus stealing, transparent background, and desktop-like stacking. Across desktop hosts, the monitor surface remains pointer-transparent: it is visible but does not participate in pointer hit-testing, so input reaches the desktop or window beneath it.

## Rationale

Iced keeps panel rendering in Rust, preserves one shared native/Web panel implementation, and consumes typed presentation data. Native desktop state remains in-process; browser/WASM state crosses only the explicit HTTP boundary. Renderer-neutral presentation semantics remain outside Iced so another frontend, such as a terminal UI, does not need to depend on Iced widgets or pixel layout.

Wayland layer-shell and X11/EWMH solve window-management semantics, not drawing. Keeping those choices outside the shared Iced view prevents protocol-specific branches from spreading through widgets, graphs, layout, or presentation.

These desktop-integration dependencies use permissive licensing and remain owned by Cargo in the single executable build.

Native tray actions are translated into platform-neutral desktop commands. In particular, a tray `Quit` action is delivered to the application lifecycle, which performs normal runtime shutdown. Platform callbacks do not terminate the process directly. This keeps the application command semantic reusable by a future Windows notification-area backend.

## Consequences

- Iced panel drawing code is shared across Linux Wayland, Linux X11, Windows, and browser/WASM delivery; native desktop integration owns window hosting, stacking, and pointer-input-region semantics while Web runtime code owns browser canvas hosting.
- Linux tray integration is shared across X11 and Wayland through StatusNotifierItem/D-Bus; it is independent of monitor-surface protocol selection.
- Native tray/menu identifiers remain platform-private. Application-visible lifecycle intent uses platform-neutral desktop commands such as `Quit`.
- Tray registration failure degrades to running without a tray and does not disable monitoring or terminate the process.
- Linux runtime protocol selection belongs to `cclover-desktop` rather than `main.rs` or the shared renderer UI.
- Sampling remains independent from rendering; renderer-neutral presentation is derived from completed shared `MonitorState` values before Iced rendering.
- Iced-specific layout and widgets are not contracts for non-desktop frontends.
- `iced_layershell` 0.19.1 currently requires `winit-core` and `winit-common` 0.31.0-beta.2 for compatibility. Those versions remain pinned until the integration dependency supports a newer compatible release.
- Pure visual changes currently require rebuilding the Rust application; reloadable UI resources are not part of this implementation.
