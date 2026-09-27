---
summary: "Defines the single-process native deployment model and target-specific dependencies."
viewpoint: deployment
concerns:
  - architecture-coherence
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

# Native Deployment

Each target builds one application executable containing the Rust application and any required compiled C++ bridge objects.

Linux and Windows builds include only their selected platform backend. The Linux executable supports both Wayland and X11 display sessions at runtime while reusing one Iced desktop frontend. Wayland uses layer-shell integration; X11 uses the normal Iced/winit X11 path plus X11/EWMH window semantics. The same Linux process registers its system tray item through StatusNotifierItem on the desktop session D-Bus; no tray helper daemon or secondary executable is introduced. External native runtime libraries depend on the selected UI toolkit and optional metric integrations.
