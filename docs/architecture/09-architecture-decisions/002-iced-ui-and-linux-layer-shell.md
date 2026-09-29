---
summary: "Records the superseded decision to share an Iced desktop frontend while isolating native desktop integration."
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

Superseded for rendering ownership by [ADR 008](008-shared-ui-tree-platform-renderers.md). The historical separation between dashboard semantics and native desktop integration informed the current architecture; active desktop-host rules live in the runtime and platform Views.

## Historical Decision

The project originally used Iced as one shared Rust panel renderer. Linux Wayland used `iced_layershell`, Linux X11 used the Iced/winit X11 path plus EWMH integration, and Windows used Iced's native window path. X11 and Wayland shared one StatusNotifierItem tray implementation rather than selecting tray behavior by display protocol.

Native desktop integration remained responsible for monitor-surface and shell policy, while renderer-neutral presentation semantics stayed outside Iced. Tray actions were translated into application lifecycle intent instead of terminating the process directly.

## Rationale at the Time

Sharing Iced avoided separate native panel implementations and kept native state typed and in-process. Layer-shell and X11/EWMH were treated as window-management concerns rather than reasons to fork dashboard semantics.

## Supersession

Windows compatibility work showed that the cross-platform graphics/window stack imposed disproportionate adapter, surface, redraw, and compatibility cost for this small read-only monitor. ADR 008 replaced shared Iced rendering with one application-specific dashboard tree plus platform-native renderers and a browser-native Web renderer.

The current Linux placement, pointer-passthrough, tray, lifecycle, and renderer responsibilities are defined by [Desktop Integration Lifecycle](../06-runtime-view/desktop-integration-lifecycle.md), [Desktop Host](../05-building-block-view/desktop-host.md), and [Linux Platform Boundary](../08-cross-cutting-concepts/linux-platform-boundary.md), not by this historical decision.
