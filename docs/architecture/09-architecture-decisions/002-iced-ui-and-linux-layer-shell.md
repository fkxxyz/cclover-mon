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

## Status

Superseded for rendering ownership by ADR 008. Linux layer-shell/X11 hosting history remains useful, but Iced is no longer the dashboard authority.

## Decision

Use Iced for the shared Rust panel frontend. Native desktop integration owns both monitor-surface hosting and platform shell integration; browser/WASM hosting owns only Web runtime and transport concerns. Neither responsibility forks dashboard drawing or presentation logic.

On Linux:

- Wayland sessions use `iced_layershell` and the layer-shell protocol.
- X11 sessions use the normal Iced/winit X11 window path plus X11/EWMH window semantics where Iced does not expose the required desktop-panel behavior.
- Runtime selection prefers Wayland when `WAYLAND_DISPLAY` is present and otherwise uses X11 when `DISPLAY` is present.
- X11 and Wayland share one StatusNotifierItem system-tray implementation over the session D-Bus; tray integration is not selected by display protocol.

Historically Windows ran the same Iced frontend through its normal native window path. ADR 008 supersedes that choice; Windows now uses the native Win32/GDI desktop path behind `cclover-desktop`.

Wayland preserves the inherited top-right placement, bottom layer, and zero exclusive zone. X11 should reproduce the same user-visible intent as closely as EWMH/window-manager semantics permit: fixed top-right placement, no decorations, no taskbar/pager entry, no focus stealing, transparent background, and desktop-like stacking. Across desktop hosts, the monitor surface remains pointer-transparent: it is visible but does not participate in pointer hit-testing, so input reaches the desktop or window beneath it.

## Rationale

Iced keeps panel rendering in Rust, preserves one shared native/Web panel implementation, and consumes typed presentation data. Native desktop state remains in-process; browser/WASM state crosses only the explicit HTTP boundary. Renderer-neutral presentation semantics remain outside Iced so another frontend, such as a terminal UI, does not need to depend on Iced widgets or pixel layout.

Wayland layer-shell and X11/EWMH solve window-management semantics, not drawing. Keeping those choices outside the shared Iced view prevents protocol-specific branches from spreading through widgets, graphs, layout, or presentation.

These desktop-integration dependencies use permissive licensing and remain owned by Cargo in the single executable build.

Native tray actions are translated into platform-neutral desktop commands. In particular, a tray `Quit` action is delivered to the application lifecycle, which performs normal runtime shutdown. Platform callbacks do not terminate the process directly. This keeps the application command semantic reusable by a future Windows notification-area backend.

## Consequences

- Historical consequence only: Iced panel drawing was shared across Linux Wayland, Linux X11, Windows, and browser/WASM delivery. ADR 008 replaces this with shared dashboard semantics plus separate renderer adapters.
- Linux tray integration is shared across X11 and Wayland through StatusNotifierItem/D-Bus; it is independent of monitor-surface protocol selection.
- Native tray/menu identifiers remain platform-private. Application-visible lifecycle intent uses platform-neutral desktop commands such as `Quit`.
- Tray registration failure degrades to running without a tray and does not disable monitoring or terminate the process.
- Linux runtime protocol selection belongs to `cclover-desktop` rather than `main.rs` or the shared renderer UI.
- Sampling remains independent from rendering; renderer-neutral presentation is derived from completed shared `MonitorState` values before renderer realization.
- Historical Iced-specific layout and widgets are not contracts for current frontends.
- Historical only: `iced_layershell` required pinned winit-compatible versions during the former Iced desktop implementation. ADR 008 removed that dependency path.
- Pure visual changes currently require rebuilding the Rust application; reloadable UI resources are not part of this implementation.
