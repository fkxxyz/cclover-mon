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

Each native target builds one application executable containing the Rust application, any required compiled C++ bridge objects, and the generated Web monitor assets.

Linux and Windows builds include only their selected platform backend. The Linux executable supports both Wayland and X11 display sessions at runtime while reusing one Iced panel implementation. Wayland uses layer-shell integration; X11 uses the normal Iced/winit X11 path plus X11/EWMH window semantics. The same Linux process registers its system tray item through StatusNotifierItem on the desktop session D-Bus; no tray helper daemon or secondary executable is introduced. External native runtime libraries depend on the selected UI toolkit and optional metric integrations.

The native executable can expose desktop, terminal, and HTTP frontends in any combination. With no frontend flag it defaults to the desktop panel; once any of `--desktop`, `--tui`, or `--http` is present, only explicitly selected frontends run. All enabled frontends share the process's single sampler and completed state stream.

The default/full native build enables the `http` and `ebpf-io` Cargo features. HTTP compiles the shared Iced panel for `wasm32-unknown-unknown`, runs `wasm-bindgen`, and embeds the resulting JavaScript/WASM bytes into the native executable. On Linux, `ebpf-io` compiles the attribution BPF programs and links libbpf. A supported minimal native build uses `--no-default-features`; it skips both optional build toolchains, rejects `--http`, and exposes process I/O attribution as disabled while leaving unrelated collectors and frontends available. When HTTP is available and enabled, the native process serves the embedded assets and read-only SSE state stream itself. The default listener is `127.0.0.1:9847`; LAN exposure requires an explicit `--http-bind` value. No Node.js runtime, external Web server, sidecar process, or additional deployed file is required.

Full Linux builds embed the eBPF programs for the backend to load at runtime. Production startup does not invoke BCC, Python, clang, LLVM, `iotop`, `nethogs`, or another monitoring executable.

NVIDIA temperature telemetry is an optional Linux runtime integration. The executable dynamically loads `libnvidia-ml.so.1` when present; packaging must not make NVML a mandatory loader dependency. Systems without the library, without NVIDIA devices, or with an unusable NVML installation run normally and simply expose no NVML-derived temperature entries. Temperature reads must not require `nvidia-smi`, a helper daemon, or elevated process authority.

Where Linux per-process I/O attribution is enabled, deployment must satisfy the kernel and privilege requirements of the chosen libbpf CO-RE fentry/fexit attachments. On supported modern kernels this means readable kernel BTF plus `CAP_BPF` and `CAP_PERFMON` (and a sufficient locked-memory limit where the kernel/libbpf combination still requires it). These are prerequisites for attribution metrics, not for unrelated collectors. Packaging may grant the narrow capabilities to the executable or use an equivalent launcher policy; it must not make unrestricted root execution the normal requirement. A compatibility path requiring broader authority must be explicit and separately justified.

Failure to satisfy those prerequisites leaves the affected I/O-attribution metric unavailable while the rest of the single application process continues to run.
