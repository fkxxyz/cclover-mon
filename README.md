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
- **Windows:** supported native collectors for CPU, memory, processes, network interfaces, physical disks, temperatures, and NVIDIA/AMD GPU telemetry. Per-process disk/network attribution is not implemented.

## Build

Requires a Rust toolchain with Cargo.

```bash
cargo build --release
```

The executable is written to:

```text
target/release/cclover-mon
```

Windows cross validation uses `cargo-xwin` through the repository validation entry point. It compiles target-specific tests and release builds for both supported targets, fixing `XWIN_ARCH=x86,x86_64` independently for every xwin invocation.

```bash
bun validate.ts windows
```

On a Windows host, run the deterministic Windows test suite with:

```bash
bun validate.ts windows-native
```

For local preflight and Linux validation, use `bun validate.ts fast` and `bun validate.ts linux`. `bun validate.ts portable` runs the host-portable fast/Linux/Windows-cross profiles; CI additionally runs `windows-native` on a Windows runner.

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

## License

`cclover-mon` is licensed under the GNU General Public License, version 2 or later (`GPL-2.0-or-later`). See [`LICENSE`](LICENSE).

Third-party components retain their own licenses. Redistributed third-party binaries, notices, and corresponding-source obligations are handled under those component licenses rather than being relicensed as `cclover-mon` code.
