# cclover-mon

A lightweight native system monitor for Linux and Windows, with interactive desktop/TUI frontends and a separate headless server artifact.

## Features

- CPU usage
- Memory and swap usage
- Top 8 processes by CPU and memory
- Network throughput for active physical interfaces
- Per-process network attribution on Linux and Windows
- Physical block-device I/O throughput
- Hardware temperatures from hwmon
- 60-sample history graphs

## Platform Support

- **Linux:** supported. Uses native `/proc` and `/sys` data sources and Wayland layer-shell placement.
- **Windows:** supported native collectors for CPU, memory, processes, network interfaces, physical disks, temperatures, NVIDIA/AMD GPU telemetry, per-process network attribution through NDU, and per-process disk attribution through FileIo ETW. NDU uses an undocumented Windows ABI, and representative real-host ETW semantic coverage remains explicitly tracked.

## Build

Requires a Rust toolchain with Cargo. Default Linux builds also require clang and libbpf development files; the resulting `ebpf-io` artifact requires libbpf 1.0.0 or newer within the `libbpf.so.1` ABI generation. `--no-default-features` omits the eBPF integration and its libbpf dependency.

```bash
cargo build --release
```

The interactive executable is written to:

```text
target/release/cclover-mon
```

Build the independent headless server product with:

```bash
cargo build --release -p cclover-server
```

Its executable is written to `target/release/cclover-mon-server`. It does not depend on the desktop/TUI crates or Linux GUI libraries.

For local inspection of the final distribution profile, use:

```bash
cargo build --profile dist
```

Published releases are produced through the repository release authority instead of
hand-assembling binaries:

```bash
bun release.ts plan
```

Pushing a tag matching the workspace version, for example `v0.1.0`, runs the release
workflow. It validates the revision, builds every declared product/target with the
`dist` profile, packages platform service assets, verifies checksums and common Git
revision identity, then publishes the complete set. A missing product/target artifact
fails the release rather than publishing a partial set.

Current published artifact names are:

```text
cclover-mon-v<VERSION>-x86_64-unknown-linux-gnu.tar.gz
cclover-mon-server-v<VERSION>-x86_64-unknown-linux-gnu.tar.gz
cclover-mon-v<VERSION>-x86_64-pc-windows-msvc.zip
cclover-mon-server-v<VERSION>-x86_64-pc-windows-msvc.zip
cclover-mon-v<VERSION>-i686-pc-windows-msvc.zip
cclover-mon-server-v<VERSION>-i686-pc-windows-msvc.zip
```

Server archives include the matching systemd or Windows service installation assets.
Windows archives also carry the applicable third-party notices and pinned source
provenance for redistributed PawnIO payloads.

Windows cross validation uses `cargo-xwin` through the repository validation entry point. It compiles target-specific tests and release builds for both supported targets, fixing `XWIN_ARCH=x86,x86_64` independently for every xwin invocation.

```bash
bun validate.ts windows
```

On a Windows host, run the deterministic Windows test suite with:

```bash
bun validate.ts windows-native
```

For local preflight and Linux validation, use `bun validate.ts fast` and `bun validate.ts linux`. Linux ABI validation requires `readelf` from binutils. On a Linux host where elevated BPF execution is available, `bun validate.ts linux-ebpf-runtime` verifies the production eBPF path and scalar map-lookup fallback; it uses the existing clang toolchain and requires root or `sudo`. Use `bun validate.ts server` for the headless product dependency/runtime/SIGTERM smoke. `bun validate.ts portable` runs the host-portable fast/Linux/Windows-cross profiles; CI additionally runs `windows-native` on a Windows runner.

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

For headless operation:

```bash
./target/release/cclover-mon-server
./target/release/cclover-mon-server --bind 0.0.0.0:9847
```

The server defaults to `127.0.0.1:9847`. `/healthz` reports HTTP liveness, while `/readyz` becomes ready after the first real sample. Linux service files are under `packaging/server/systemd/`; Windows SCM installation support is under `packaging/server/windows/`.

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
