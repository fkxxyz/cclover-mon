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

Each native target builds one application executable containing the Rust application, required native C objects, any C++ compatibility shims, and generated Web monitor assets.

Published distribution artifacts use the Cargo `dist` profile, which inherits the optimized `release` profile and sets `panic = "abort"`. A Rust panic is treated as an unrecoverable program defect, so shipped binaries terminate the process instead of paying for unwinding support. Normal development and validation continue to use their existing profiles and panic semantics.

Linux and Windows builds include only their selected platform backend. Both native desktop paths consume the same `cclover-ui::NativeScene` contract. Windows uses Win32/GDI. Linux uses native Wayland layer-shell or X11 hosting with shared Cairo drawing. Neither native desktop path depends on Iced, winit, or wgpu. The same Linux process registers its system tray item through StatusNotifierItem on the desktop session D-Bus; no tray helper daemon or secondary executable is introduced. External native runtime libraries depend on the selected renderer and optional metric integrations.

Windows deployment supports both `x86_64-pc-windows-msvc` and `i686-pc-windows-msvc`. Platform integration code must therefore remain pointer-width correct rather than assuming 64-bit Win32 handle or style types.

The Windows distribution remains a single published `cclover-mon.exe` when PawnIO-backed sensors are enabled. Build preparation embeds only the verified production driver package required by the target plus signed Pawn modules used by implemented collectors; upstream installers, debug/test payloads, unrelated architectures, and `PawnIOLib.dll` are not runtime artifacts.

One repository-owned dependency declaration pins PawnIO provenance, target/OS mappings, selected component digests, module versions, and required module digests. Preparation owns network access and a user-scoped cache outside the worktree; Rust compilation embeds only prepared verified bytes.

PawnIO provisioning is machine-level, idempotent, and separate from sampling; runtime access is process-level and follows the installed device policy. Unsupported or denied combinations degrade only PawnIO-backed sensors. Detailed compatibility, elevation, upgrade, signature, cleanup, supply-chain, and redistribution rules are owned by [ADR 009](../09-architecture-decisions/009-windows-pawnio-provisioning.md). Project-owned source is `GPL-2.0-or-later`; vendored source and embedded third-party payloads retain their upstream licensing obligations. A Windows target containing GPL-2.0-only Linux hwmon source is distributed under GPLv2-compatible terms for the combined work.

Selected Linux hwmon source is vendored into the repository, so ordinary builds do not clone or fetch the Linux kernel. Upstream synchronization is a separate maintenance action using an external user-scoped partial/sparse Git cache and an exact pinned Linux commit; build inputs remain the checked-in source snapshot. Imported GNU C units compile as Windows objects through the compatibility boundary and link into the same executable under Cargo's build authority.

Every cargo-xwin invocation for this project must carry `XWIN_ARCH=x86,x86_64` on that exact command line, regardless of whether the immediate Rust target is 32-bit or 64-bit. Do not rely on an earlier `export`, shell/session state, or profile configuration. The setting controls which Microsoft CRT/Windows SDK architectures cargo-xwin materializes, not the Rust target itself. Keeping this value stable gives both supported Windows targets one shared cache layout and avoids cargo-xwin falling back to its different default architecture set or re-preparing the cache when the setting changes. Build instructions and automation must not omit, narrow, or otherwise vary this value per target.

The Windows executable uses the GUI PE subsystem so desktop launches do not create a console window. At startup it attaches to an existing parent console when available; terminal-facing modes such as `--tui` and diagnostic CLI commands allocate a console only when no parent console exists. Automatic frontend selection observes the attached parent console rather than allocating a console merely to make TUI appear available. This preserves the single-executable, composable-frontend model without making desktop-only launches behave like console applications.

The native executable can expose desktop, terminal, and HTTP frontends in any combination. With no frontend flag the application composition root selects the desktop frontend when the native desktop host reports an interactive desktop environment, otherwise selects TUI when stdin and stdout are interactive terminals, and otherwise fails with an explicit-frontend diagnostic. Desktop takes precedence when both capabilities are present. Once any of `--desktop`, `--tui`, or `--http` is present, automatic selection is disabled and only explicitly selected frontends run. Capability detection does not provide runtime fallback: failure after a frontend has been selected is reported as that frontend's startup failure. All enabled frontends share the process's single sampler and completed state stream.

The default/full native build enables the `http` and `ebpf-io` Cargo features. HTTP uses `cclover-web-ui` inside the native process to lower the shared `cclover-ui` dashboard tree to HTML/SVG and serves a small static JavaScript EventSource adapter plus CSS. It does not build or embed WebAssembly and requires no `wasm32` target, `wasm-bindgen`, Node.js runtime, external Web server, sidecar process, or additional deployed file. On Linux, `ebpf-io` compiles the attribution BPF programs and links libbpf. A supported minimal native build uses `--no-default-features`; it skips the optional HTTP renderer and eBPF build toolchain, rejects `--http`, and exposes process I/O attribution as disabled while leaving unrelated collectors and frontends available. When HTTP is available and enabled, the native process serves the page, rendered-dashboard SSE stream, and public read-only API itself. The default listener is `127.0.0.1:9847`; LAN exposure requires an explicit `--http-bind` value.

Full Linux builds embed the eBPF programs for the backend to load at runtime. Production startup does not invoke BCC, Python, clang, LLVM, `iotop`, `nethogs`, or another monitoring executable.

NVIDIA GPU telemetry is an optional Linux runtime integration. The executable dynamically loads `libnvidia-ml.so.1` when present; packaging must not make NVML a mandatory loader dependency. Systems without the library, without NVIDIA devices, or with an unusable NVML installation run normally and expose no NVML-derived GPU entries. Supported utilization, VRAM, temperature, power, core-clock, and fan fields are read in-process; collection must not require `nvidia-smi`, a helper daemon, or elevated process authority.

Windows vendor GPU telemetry is likewise optional and dynamically loads the runtime supplied by the installed display driver: NVIDIA `nvml.dll`, or AMD `atiadlxx.dll` / `atiadlxy.dll` according to process bitness. cclover-mon redistributes neither vendor runtime. Missing or unusable vendor libraries leave only that source unsupported without affecting other Windows collectors. The Windows process retains one session/device set per available vendor source rather than loading or rediscovering driver libraries per sample.

Where Linux per-process I/O attribution is enabled, deployment must satisfy the kernel and privilege requirements of the chosen libbpf CO-RE fentry/fexit attachments. On supported modern kernels this means readable kernel BTF plus `CAP_BPF` and `CAP_PERFMON` (and a sufficient locked-memory limit where the kernel/libbpf combination still requires it). These are prerequisites for attribution metrics, not for unrelated collectors. Packaging may grant the narrow capabilities to the executable or use an equivalent launcher policy; it must not make unrestricted root execution the normal requirement. A compatibility path requiring broader authority must be explicit and separately justified.

Failure to satisfy those prerequisites leaves the affected I/O-attribution metric unavailable while the rest of the single application process continues to run.
