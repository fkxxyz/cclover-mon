---
summary: "Defines the native desktop host boundary and Scene realization responsibilities."
viewpoint: static
concerns:
  - architecture-coherence
  - portability
  - maintainability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - platform
    - native-bridge
---

# Desktop Host

`cclover-desktop` owns native monitor-surface hosting and shell integration. It bridges the latest-state receiver to `Scene`, exposes the narrow scene/state-consumption ABI to platform-native code, and translates native shell intent into platform-neutral lifecycle signals.

Linux selects Wayland layer-shell or X11 at runtime while sharing Cairo primitive execution; both display protocols share one StatusNotifierItem tray backend. Windows owns Win32 window/message/tray integration and GDI drawing. Shared presentation and UI code do not select display protocols, manipulate native windows, or own shell handles.

`Scene` owns requested surface geometry in device-independent logical units. Native primitive invalidation semantics also have one shared authority: primitive equality defines conservative visual identity, primitive bounds define affected logical geometry, and the Rust desktop bridge derives static revisions, full-redraw fallback, and merged dynamic damage before crossing the native ABI. Native hosts consume that invalidation result; they do not independently hash commands or derive primitive damage bounds.

Native hosts realize logical scene geometry into the active protocol's device pixels and own display scale, font realization, drawing APIs, surface/buffer lifecycle, placement, pointer passthrough, taskbar/Alt+Tab policy, Z-order, and shell recovery. Font metrics stay renderer-local and do not feed shared layout. Native hosts do not independently rebuild dashboard row layout.

Desktop shell actions cross the boundary only as small platform-neutral lifecycle signals such as quit. Lifecycle signals are wakeable events, not flags polled as part of scene/state consumption, and worker shutdown must be explicitly interruptible rather than depend on periodic timeout checks. Linux keeps state-ready and quit wake sources distinct so state backpressure cannot lose a lifecycle event; Windows maps lifecycle directly onto its native message/window lifetime. D-Bus objects, tray/menu identifiers, Win32 handles, layer-shell operations, X11/EWMH details, and host-side resize mechanics remain platform-private.

Native scheduling and lifecycle decisions are behaviorally testable below the OS event primitives. The Rust state bridge owns latest-state coalescing, wake delivery, consumption, and joined shutdown semantics. Native adapters delegate only their real semantic decisions to small compiled policy modules: Wayland owns layer-surface/output lifecycle state, X11 owns running/dirty state, and Win32 owns state-message relay/refresh policy. Those modules contain no compositor, X server, or Win32 calls, so deterministic tests execute the same policy code used by production hosts without simulating complete window systems. Real desktop checks remain integration evidence for protocol/API behavior rather than the only proof of lifecycle correctness.

Linux tray protocol declarations must match implemented protocol capabilities. When the exported D-BusMenu version supports both single and grouped calls, `Event`/`EventGroup` and `AboutToShow`/`AboutToShowGroup` remain implemented together, with single and grouped activation converging on one lifecycle handler.
