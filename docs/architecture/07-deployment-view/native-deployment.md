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

Linux and Windows builds include only their selected platform backend. Both native desktop paths consume the same `cclover-ui::NativeScene` contract. Windows uses Win32/GDI. Linux uses native Wayland layer-shell or X11 hosting with shared Cairo drawing. Neither native desktop path depends on Iced, winit, or wgpu. The same Linux process registers its system tray item through StatusNotifierItem on the desktop session D-Bus; no tray helper daemon or secondary executable is introduced. External native runtime libraries depend on the selected renderer and optional metric integrations.

Windows deployment supports both `x86_64-pc-windows-msvc` and `i686-pc-windows-msvc`. Platform integration code must therefore remain pointer-width correct rather than assuming 64-bit Win32 handle or style types.

Every cargo-xwin invocation for this project must carry `XWIN_ARCH=x86,x86_64` on that exact command line, regardless of whether the immediate Rust target is 32-bit or 64-bit. Do not rely on an earlier `export`, shell/session state, or profile configuration. The setting controls which Microsoft CRT/Windows SDK architectures cargo-xwin materializes, not the Rust target itself. Keeping this value stable gives both supported Windows targets one shared cache layout and avoids cargo-xwin falling back to its different default architecture set or re-preparing the cache when the setting changes. Build instructions and automation must not omit, narrow, or otherwise vary this value per target.

The Windows executable uses the GUI PE subsystem so the default desktop mode does not create a console window. At startup it attaches to an existing parent console when available; terminal-facing modes such as `--tui` and diagnostic CLI commands allocate a console only when no parent console exists. This preserves the single-executable, composable-frontend model without making desktop-only launches behave like console applications.

The native executable can expose desktop, terminal, and HTTP frontends in any combination. With no frontend flag it defaults to the desktop panel; once any of `--desktop`, `--tui`, or `--http` is present, only explicitly selected frontends run. All enabled frontends share the process's single sampler and completed state stream.

The default/full native build enables the `http` and `ebpf-io` Cargo features. HTTP compiles the Web/WASM Iced adapter for `wasm32-unknown-unknown`; that adapter consumes the same `cclover-ui` dashboard tree, then `wasm-bindgen` output is embedded into the native executable. Iced/WGPU therefore remain Web build dependencies but are absent from native desktop rendering. On Linux, `ebpf-io` compiles the attribution BPF programs and links libbpf. A supported minimal native build uses `--no-default-features`; it skips both optional build toolchains, rejects `--http`, and exposes process I/O attribution as disabled while leaving unrelated collectors and frontends available. When HTTP is available and enabled, the native process serves the embedded assets and read-only SSE state stream itself. The default listener is `127.0.0.1:9847`; LAN exposure requires an explicit `--http-bind` value. No Node.js runtime, external Web server, sidecar process, or additional deployed file is required.

Full Linux builds embed the eBPF programs for the backend to load at runtime. Production startup does not invoke BCC, Python, clang, LLVM, `iotop`, `nethogs`, or another monitoring executable.

NVIDIA GPU telemetry is an optional Linux runtime integration. The executable dynamically loads `libnvidia-ml.so.1` when present; packaging must not make NVML a mandatory loader dependency. Systems without the library, without NVIDIA devices, or with an unusable NVML installation run normally and expose no NVML-derived GPU entries. Supported utilization, VRAM, temperature, power, core-clock, and fan fields are read in-process; collection must not require `nvidia-smi`, a helper daemon, or elevated process authority.

Where Linux per-process I/O attribution is enabled, deployment must satisfy the kernel and privilege requirements of the chosen libbpf CO-RE fentry/fexit attachments. On supported modern kernels this means readable kernel BTF plus `CAP_BPF` and `CAP_PERFMON` (and a sufficient locked-memory limit where the kernel/libbpf combination still requires it). These are prerequisites for attribution metrics, not for unrelated collectors. Packaging may grant the narrow capabilities to the executable or use an equivalent launcher policy; it must not make unrestricted root execution the normal requirement. A compatibility path requiring broader authority must be explicit and separately justified.

Failure to satisfy those prerequisites leaves the affected I/O-attribution metric unavailable while the rest of the single application process continues to run.
