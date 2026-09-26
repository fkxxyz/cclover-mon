# cclover-mon - Project Guidelines

## What This Project Does

`cclover-mon` is a low-overhead native desktop system monitor for Linux and Windows. It keeps one shared Rust model and Iced UI while isolating native collection and desktop integration behind platform backends.

## Architecture

**Primary stack**: Rust 2024, Cargo, Iced 0.14. Linux Wayland placement uses `iced_layershell` 0.19.1.

**Dependency direction**:

```text
app / UI
  ↓
core: model + history + aggregation
  ↓
platform: Linux | Windows
  ↓
OS native interfaces
```

Ownership rules:

- `core` owns platform-neutral metric types, delta/rate derivation, Top-N aggregation, bounded history, and sampling contracts.
- `platform` owns OS-specific collection and desktop integration. Platform API types stay inside this layer.
- `ui` renders shared `MonitorState`; it does not collect metrics or call platform APIs.
- Native data stays typed and in-process. Do not introduce internal JSON or frontend/backend IPC for metric flow.
- Prefer native collection over periodic subprocess polling. Linux sources should use `/proc`, `/sys`, netlink, ioctl, sockets, or D-Bus as appropriate.
- Keep sampling cadence independent from rendering cadence.
- Keep history bounded with fixed-capacity ring-buffer semantics.
- C++ is allowed only for C++-only SDKs and must stay behind a narrow C ABI linked into the same executable.

## Project Structure

```text
src/app.rs                 application state, sampling subscription, UI update flow
src/core/model.rs          shared typed snapshots and history model
src/core/sampler.rs        delta/rate derivation, Top-N, sampling state
src/core/history.rs        bounded history updates
src/platform/linux.rs      Linux native collectors
src/platform/windows.rs    Windows backend; collector is currently a placeholder
src/ui/mod.rs              shared Iced presentation
src/ui/graph.rs            history graph rendering
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

Run the complete project validation set after implementation changes:

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release
bun archdoc.ts check
```

For Linux UI or window-placement changes, also perform a real Wayland runtime check. If the shell does not inherit the desktop environment, locate the compositor socket under `/run/user/$(id -u)/wayland-*` and set the matching `XDG_RUNTIME_DIR` and `WAYLAND_DISPLAY` before launching `target/release/cclover-mon`.

## Common Pitfalls

- `iced_layershell` 0.19.1 currently requires exact `winit-core` and `winit-common` `0.31.0-beta.2` compatibility pins. Do not relax these pins without build and runtime validation.
- Linux is implemented and runtime-validated; the Windows collector is still a placeholder. Do not describe Windows metric parity as complete.
- Static compilation is insufficient for UI changes. Previous runtime checks caught layout overlap and virtual block devices that passed Rust tests and Clippy.
- The Linux panel intentionally uses top-right anchoring, bottom layer, and zero exclusive zone. Preserve these semantics unless the product behavior is intentionally changed.

## Build and Run

Release build:

```bash
cargo build --release
```

Development launch:

```bash
cargo run --release
```

Built executable:

```text
target/release/cclover-mon
```

Each target produces one application executable containing the selected platform backend and any required native bridge objects.
