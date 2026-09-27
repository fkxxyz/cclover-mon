---
summary: "Defines the single-process native deployment model and target-specific dependencies."
viewpoint: deployment
concerns:
  - architecture-coherence
  - performance
  - portability
  - maintainability
  - security
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

Linux eBPF programs are compiled at build time and embedded into the application executable for the Linux backend to load. Production startup does not invoke BCC, Python, clang, LLVM, `iotop`, `nethogs`, or another monitoring executable.

Where Linux per-process I/O attribution is enabled, deployment must satisfy the kernel and privilege requirements of the chosen libbpf CO-RE fentry/fexit attachments. On supported modern kernels this means readable kernel BTF plus `CAP_BPF` and `CAP_PERFMON` (and a sufficient locked-memory limit where the kernel/libbpf combination still requires it). These are prerequisites for attribution metrics, not for unrelated collectors. Packaging may grant the narrow capabilities to the executable or use an equivalent launcher policy; it must not make unrestricted root execution the normal requirement. A compatibility path requiring broader authority must be explicit and separately justified.

Failure to satisfy those prerequisites leaves the affected I/O-attribution metric unavailable while the rest of the single application process continues to run.
