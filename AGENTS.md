# cclover-mon - Project Guidelines

## What This Project Does

`cclover-mon` is a low-overhead native system monitor for Linux and Windows. It keeps one shared Rust metric model, renderer-neutral presentation semantics, and one renderer-neutral graphical dashboard definition while isolating native collection and desktop rendering/integration behind platform backends.

## Architecture

**Primary stack**: Rust 2024 and Cargo. `cclover-ui` owns the graphical dashboard definition. Native desktop rendering uses Win32/GDI on Windows and Wayland/X11 + Cairo on Linux. Iced 0.14 remains only in the Web/WASM renderer adapter.

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
- `cclover-web-ui` is the Web/WASM Iced renderer adapter. Iced is not dashboard authority and must not be reintroduced into native desktop dependencies.
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
src/platform/windows.rs    Windows backend; collector is currently a placeholder
crates/cclover-presentation/src/lib.rs  renderer-neutral dashboard presentation model and formatting
crates/cclover-tui/src/lib.rs           terminal frontend rendering and terminal lifecycle
crates/cclover-ui/src/lib.rs            shared graphical dashboard tree, style tokens, geometry
crates/cclover-web-ui/src/lib.rs       Web/WASM Iced renderer adapter
crates/cclover-web-ui/src/graph.rs     Web graph primitive adapter
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

## Validation

Run the fast architecture gate during implementation:

```bash
bun archgate.ts
```

It checks source dependency direction only and is intentionally millisecond-scale; do not add compilation, formatting, Clippy, or runtime checks to this gate.

Run the complete project validation set after implementation changes:

```bash
bun archgate.ts
bun test archgate.test.ts
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo build --release
cargo test -p cclover-mon --no-default-features
cargo clippy -p cclover-mon --no-default-features --all-targets -- -D warnings
bun archdoc.ts check
```

For Linux UI or window-placement changes, also perform a real Wayland runtime check. If the shell does not inherit the desktop environment, locate the compositor socket under `/run/user/$(id -u)/wayland-*` and set the matching `XDG_RUNTIME_DIR` and `WAYLAND_DISPLAY` before launching `target/release/cclover-mon`.

## Common Pitfalls

- Native Linux desktop rendering requires Cairo, X11/Xext, Wayland client libraries, and the checked-in generated layer-shell protocol sources. Do not reintroduce Iced/winit/wgpu to avoid native host work.
- Every `cargo xwin` build must use exactly `XWIN_ARCH=x86,x86_64`, including builds targeting only `i686-pc-windows-msvc` or only `x86_64-pc-windows-msvc`. Do not omit it or switch to a per-target value: cargo-xwin's default architecture set differs, and changing this setting can force CRT/SDK cache re-download/re-splat work.
- Linux is implemented and runtime-validated; the Windows collector is still a placeholder. Do not describe Windows metric parity as complete.
- Static compilation is insufficient for UI changes. Previous runtime checks caught layout overlap and virtual block devices that passed Rust tests and Clippy.
- The Linux panel intentionally uses top-right anchoring, bottom layer, and zero exclusive zone. Preserve these semantics unless the product behavior is intentionally changed.

## Build and Run

Release build (full artifact, including HTTP/Web and eBPF attribution):

```bash
cargo build --release
```

Minimal native validation without the HTTP/WASM or eBPF/libbpf build toolchains:

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

Windows cross-builds:

```bash
XWIN_ARCH=x86,x86_64 cargo xwin build --release --target x86_64-pc-windows-msvc
XWIN_ARCH=x86,x86_64 cargo xwin build --release --target i686-pc-windows-msvc
```

Each target produces one application executable containing the selected platform backend and any required native bridge objects.
