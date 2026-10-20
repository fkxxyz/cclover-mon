# Static Server Device Trial

How to gather runtime evidence for the static `cclover-mon-server` (`aarch64-unknown-linux-musl`, no eBPF) on a real device, and what has been established so far. Build it as described in the README. `bun validate.ts server-static` proves only that the artifact is self-contained; it does not execute it. Device-specific addresses, accounts, and deployment configuration do not belong in this repository.

## Trial procedure

Run from memory-backed storage on a loopback port; nothing is installed or configured on the device.

```bash
scp target/aarch64-unknown-linux-musl/dist/cclover-mon-server <device>:/tmp/cclover-trial
ssh <device> '/tmp/cclover-trial --version'            # expect "features: none"
ssh <device> '/tmp/cclover-trial --bind 127.0.0.1:19847 &'
ssh <device> 'wget -qO- http://127.0.0.1:19847/readyz'  # "ready" after the first sample
ssh <device> 'wget -qO- http://127.0.0.1:19847/api/v1/state'
```

Inspect `disks`, `networks`, `temperatures`, and `process_*_io` (expect `disabled` in this build). Remove `/tmp/cclover-trial` and stop the process afterwards. Verify that it is gone from `/proc/<pid>`; `pgrep -f` can match the invoking shell's own command line.

CPU and memory: BusyBox may lack `getconf`, so derive the tick rate from `/proc/stat` (total ticks per second divided by the CPU count), then divide the process's `utime + stime` delta by tick rate times wall time. Sample at least 60 seconds idle, and once with one `/events` subscriber, because rendering is demand-driven.

## Evidence recorded

| Device class | Result |
| --- | --- |
| aarch64 OpenWrt router, musl, 4 cores, Linux 6.12 | Ready within seconds; 5 threads. Idle: 1.60% of one core, 1.66 MB RSS (96 ticks over 60 s at 100 Hz). One SSE subscriber: 2.09% of one core, 1.77 MB RSS. The cost is not attributed to a sampler stage, and no project budget has been compared with it. |

Observed on that device and relevant to design decisions:

- Verified: `/sys/kernel/btf/vmlinux` and `/proc/config.gz` are absent, so eBPF attribution cannot attach there even in a build that includes it.
- Inferred, not tested: the BPF system call is available (BPF sysctls exist). No BPF program was loaded.
- Observed: with NVML's `dlopen` in a static musl executable, GPU telemetry reports `unavailable` and the process does not fail. The exact `dlopen` behavior was not inspected.
- Collector results: temperature from CPU thermal, NVMe, and wireless hwmon sources; physical disks after the selection rules in ADR 011; network ports after DSA-conduit exclusion.

## Local verification limits

- A build script compiles native C objects even for `cargo check`, so type-checking a Windows target needs the MSVC toolchain (`lib.exe`) and type-checking the Linux eBPF path for aarch64 needs a target C compiler and libbpf headers. Use `bun validate.ts windows` where `cargo-xwin` is provisioned, or CI.
- The eBPF path has no aarch64 compile or runtime evidence; see Architecture Risks.
