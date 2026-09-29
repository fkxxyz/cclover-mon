---
summary: "Defines the native desktop host boundary and NativeScene realization responsibilities."
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

`cclover-desktop` owns native monitor-surface hosting and shell integration. It bridges the latest-state receiver to `NativeScene`, exposes the narrow scene/poll ABI to platform-native code, and translates native shell intent into platform-neutral lifecycle signals.

Linux selects Wayland layer-shell or X11 at runtime while sharing Cairo primitive execution; both display protocols share one StatusNotifierItem tray backend. Windows owns Win32 window/message/tray integration and GDI drawing. Shared presentation and UI code do not select display protocols, manipulate native windows, or own shell handles.

`NativeScene` owns requested surface geometry. Native hosts realize that geometry using the active protocol and own font realization/measurement, drawing APIs, surface/buffer lifecycle, placement, pointer passthrough, taskbar/Alt+Tab policy, Z-order, and shell recovery. They return text extents but do not independently rebuild dashboard row layout.

Desktop shell actions cross the boundary only as small platform-neutral lifecycle signals such as quit. D-Bus objects, tray/menu identifiers, Win32 handles, layer-shell operations, X11/EWMH details, and host-side resize mechanics remain platform-private.
