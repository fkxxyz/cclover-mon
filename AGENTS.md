# cclover-mon - Project Guidelines

## What This Project Does

`cclover-mon` is a low-overhead native system monitor for Linux and Windows. It keeps one shared Rust metric model, renderer-neutral presentation semantics, and one renderer-neutral graphical dashboard definition while isolating native collection and desktop rendering/integration behind platform backends.

## Architecture

**Primary stack**: Rust 2024 and Cargo. `cclover-ui` owns the graphical dashboard definition. Native desktop rendering uses Win32/GDI on Windows and Wayland/X11 + Cairo on Linux. Web rendering lowers the shared dashboard tree to HTML/SVG/CSS in native Rust and uses a thin browser EventSource/DOM adapter.

**Runtime metric flow**:

```text
OS native interfaces
  ↓
platform: Linux | Windows
  ↓
core: sampling + model + history + aggregation
  ↓
presentation
  ↓
frontend
```

**Static source dependency boundary**:

```text
app composition root ───────→ ui frontend ───────→ presentation ───────→ core
        │                                                               ↑
        ├──────────────────────────→ presentation ───────────────────────┘
        ├──────────────────────────────────────────────────────────────→ core
        └──────────────────────────→ platform ──────────────────────────┘
```

Ownership rules:

- `core` owns platform-neutral metric types, delta/rate derivation, Top-N aggregation, bounded history, and sampling contracts, including `Collector`.
- `platform` owns OS-specific collection; the `cclover-desktop` package owns OS-specific native desktop hosting/integration. Platform collectors implement core-owned sampling contracts and return core-owned platform-neutral snapshots. `core` must not depend on either native boundary.
- `presentation` derives renderer-neutral dashboard semantics from shared `MonitorState`; it does not depend on renderer libraries, terminal libraries, platform APIs, or app messages.
- `cclover-ui` owns cross-renderer graphical dashboard structure, visual tokens, graph policy, and shared geometry. Renderers consume it and must not independently rebuild monitor cards.
- `cclover-web-ui` is the server-side Web renderer adapter. It lowers the shared dashboard tree to HTML/SVG/CSS; browser JavaScript is transport/DOM glue and must not reconstruct dashboard semantics.
- Every `cargo xwin ...` invocation must include `XWIN_ARCH=x86,x86_64` on that exact command line. Do not rely on `export`, shell state, profile configuration, or a previous command. This applies to `build`, `check`, `test`, and any other cargo-xwin subcommand.
- Native data stays typed and in-process. Do not introduce internal JSON or frontend/backend IPC for metric flow.
- Prefer native collection over periodic subprocess polling. Linux sources should use `/proc`, `/sys`, netlink, ioctl, sockets, or D-Bus as appropriate.
- Keep sampling cadence independent from rendering cadence.
- Keep history bounded with fixed-capacity ring-buffer semantics.
- C++ is allowed only for C++-only SDKs and must stay behind a narrow C ABI linked into the same executable.

## Project Structure

```text
crates/cclover-desktop/src/linux.rs    Linux Wayland/X11 hosting and tray integration
crates/cclover-desktop/src/native.rs   shared NativeScene FFI bridge
crates/cclover-desktop/src/windows.rs  Windows native host bridge
crates/cclover-desktop/native/windows_host.c  Win32/GDI window, drawing, and tray host
crates/cclover-desktop/native/linux_host.c    Wayland/X11 + Cairo host/drawing
crates/cclover-core/src/model.rs    shared typed snapshots and history model
crates/cclover-core/src/sampler.rs  delta/rate derivation, Top-N, sampling state
crates/cclover-core/src/history.rs  bounded history updates
src/platform/linux/        Linux native collectors split by metric responsibility
src/platform/windows.rs    Windows backend composition; metric collectors live under src/platform/windows/
crates/cclover-presentation/src/lib.rs  renderer-neutral dashboard presentation model and formatting
crates/cclover-tui/src/lib.rs           terminal frontend rendering and terminal lifecycle
crates/cclover-ui/src/lib.rs            shared graphical dashboard tree, style tokens, geometry
crates/cclover-web-ui/src/lib.rs       server-side HTML/SVG/CSS renderer boundary
docs/architecture/         architecture Views and governance data
archdoc.ts                 architecture documentation navigator and validator
```

## Architecture Workflow

Before repository work, run:

```bash
bun archdoc.ts --help
```

Use `archdoc` to discover the Views relevant to the task before changing implementation:

```bash
bun archdoc.ts choices --stakeholder developer
bun archdoc.ts views --stakeholder developer --concern <concern> --activity <activity> --facet area=<area> [--viewpoint <viewpoint>]
```

Read the returned Markdown files before making the change. When architecture documentation changes, validate it with:

```bash
bun archdoc.ts check
```

## Development Rules

- Preserve product behavior when porting from the legacy Quickshell monitor, but implement it according to this repository's architecture rather than copying the old code structure.
- Add new metrics by extending the shared typed model and core derivation first, then implement platform collectors, then render them in UI.
- Keep Linux device discovery capability-based where practical. For example, physical block devices are identified through sysfs capability (`/sys/block/<dev>/device`) rather than maintained name blacklists.
- Treat unavailable metrics as unavailable data, not fabricated zeroes, unless zero is the correct semantic value.
- Keep platform-specific names, handles, structs, and APIs out of shared UI and core-facing contracts.
- Preserve the single-process, single-executable deployment model for each target platform.
- When updating Windows hardware-telemetry compatibility knowledge from LibreHardwareMonitor, read `docs/architecture/09-architecture-decisions/010-windows-hardware-telemetry-upstream.md` and follow `docs/maintenance/librehardwaremonitor-sync.md`. Run `bun lhm-sync.ts status <checkout>` before changing derived hardware support, and never auto-replace production register/I/O algorithms from upstream.

## Validation

`validate.ts` is the repository validation-plan authority. Keep validation command composition there; documentation and CI should invoke profiles rather than copy the underlying Cargo/Bun command list.

Run the fast deterministic profile during implementation:

```bash
bun validate.ts fast
```

`archgate.ts` remains the millisecond-scale source-dependency gate inside that profile. It may be run directly while editing, but do not add compilation, formatting, Clippy, or runtime checks to `archgate` itself.

Run Linux build/test/lint validation with:

```bash
bun validate.ts linux
```

Run both supported Windows cross-builds and compile target-specific tests with:

```bash
bun validate.ts windows
```

Run Windows-host deterministic tests with:

```bash
bun validate.ts windows-native
```

Run the real-Windows FileIo ETW disk-attribution smoke with sufficient ETW session-control authority:

```bash
bun validate.ts windows-etw-runtime
```

That profile uses the production probe with a known local write workload and a pre-opened file; it is runtime evidence, not a replacement for the broader ETW semantic-validation debt. See `docs/maintenance/windows-native-validation.md`.

Run the real-browser Web smoke test only when Chromium/Chrome is available:

```bash
bun validate.ts web-browser
```

`web-browser` is intentionally separate from `fast`, `linux`, and `portable`; ordinary development and builds do not require a browser. CI provisions Chromium/Chrome explicitly for this profile.

`bun validate.ts portable` runs the host-portable fast/Linux/Windows-cross profiles when the required Linux and cargo-xwin toolchains are available. Windows-host execution remains a separate `windows-native` profile because it requires a Windows runner. GitHub Actions invokes these same profiles; do not maintain a separate CI-only validation command set.

The Windows cross profile sets `XWIN_ARCH=x86,x86_64` on every individual `cargo xwin` subprocess. Do not invoke cargo-xwin from new automation outside this validation authority unless the same per-invocation rule is preserved.

For Linux UI or window-placement changes, also perform a real Wayland runtime check. If the shell does not inherit the desktop environment, locate the compositor socket under `/run/user/$(id -u)/wayland-*` and set the matching `XDG_RUNTIME_DIR` and `WAYLAND_DISPLAY` before launching `target/release/cclover-mon`. Runtime compositor/shell checks are evidence, not substitutes for deterministic validation.

## Common Pitfalls

- Native Linux desktop rendering requires Cairo, X11/Xext, Wayland client libraries, and the checked-in generated layer-shell protocol sources. Do not reintroduce Iced/winit/wgpu to avoid native host work.
- Every `cargo xwin` build must use exactly `XWIN_ARCH=x86,x86_64`, including builds targeting only `i686-pc-windows-msvc` or only `x86_64-pc-windows-msvc`. Do not omit it or switch to a per-target value: cargo-xwin's default architecture set differs, and changing this setting can force CRT/SDK cache re-download/re-splat work.
- Linux is implemented and runtime-validated. Windows native collectors for CPU, memory, processes, network interfaces, physical disks, temperatures, NVIDIA/AMD GPU telemetry, per-process network attribution through NDU, and per-process disk attribution through FileIo ETW are implemented. Windows ETW disk attribution is cross-build validated but still requires representative real-Windows runtime validation of event schema/completion-byte semantics. NDU uses an undocumented Windows ABI; preserve the isolated wrapper and keep real-Windows compatibility and end-to-end acceptance gaps aligned with the corresponding technical-debt records.
- Static compilation is insufficient for UI changes. Previous runtime checks caught layout overlap and virtual block devices that passed Rust tests and Clippy.
- The Linux panel intentionally uses top-right anchoring, bottom layer, and zero exclusive zone. Preserve these semantics unless the product behavior is intentionally changed.

## Build and Run

Release build (full artifact, including HTTP/Web and eBPF attribution):

```bash
cargo build --release
```

Minimal native validation without the HTTP renderer or eBPF/libbpf build toolchain:

```bash
cargo check --no-default-features
```

Development launch:

```bash
cargo run --release
```

Built executable:

```text
target/release/cclover-mon
```

Windows cross-build validation:

```bash
bun validate.ts windows
```

The profile builds both `x86_64-pc-windows-msvc` and `i686-pc-windows-msvc`, with `XWIN_ARCH=x86,x86_64` applied independently to each cargo-xwin invocation. Each target produces one application executable containing the selected platform backend and any required native bridge objects.
