# cclover-mon

A lightweight native desktop system monitor with a compact always-on-screen panel.

## Features

- CPU usage
- Memory and swap usage
- Top 8 processes by CPU and memory
- Network throughput for active physical interfaces
- Physical block-device I/O throughput
- Hardware temperatures from hwmon
- 60-sample history graphs

## Platform Support

- **Linux:** supported. Uses native `/proc` and `/sys` data sources and Wayland layer-shell placement.
- **Windows:** backend scaffold exists, but metric collection is not implemented yet.

## Build

Requires a Rust toolchain with Cargo.

```bash
cargo build --release
```

The executable is written to:

```text
target/release/cclover-mon
```

Windows cross-builds can use `cargo-xwin`. This project fixes `XWIN_ARCH=x86,x86_64` for all xwin builds so the same CRT/SDK cache layout is reused for both supported targets. Do not omit or change `XWIN_ARCH` between builds; cargo-xwin's default architecture set differs and can trigger unnecessary cache re-download/re-splat work.

```bash
XWIN_ARCH=x86,x86_64 cargo xwin build --release --target x86_64-pc-windows-msvc
XWIN_ARCH=x86,x86_64 cargo xwin build --release --target i686-pc-windows-msvc
```

The 32-bit executable is written to:

```text
target/i686-pc-windows-msvc/release/cclover-mon.exe
```

## Run

```bash
cargo run --release
```

Or run the built executable directly:

```bash
./target/release/cclover-mon
```

### Wayland

When launched from a terminal inside the desktop session, the required Wayland environment is normally already available.

If launching from a shell that did not inherit the desktop session environment, locate the compositor socket:

```bash
ls /run/user/$(id -u)/wayland-*
```

Then set the matching values before launching, for example:

```bash
export XDG_RUNTIME_DIR=/run/user/$(id -u)
export WAYLAND_DISPLAY=wayland-1
./target/release/cclover-mon
```

Use the actual socket name on the system; it is not necessarily `wayland-1`.

## Documentation

Architecture documentation is under [`docs/architecture/`](docs/architecture/).
