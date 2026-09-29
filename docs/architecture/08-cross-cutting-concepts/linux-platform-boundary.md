---
summary: "Defines Linux-specific collection and desktop-integration boundaries."
viewpoint: static
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
    - platform
---

# Linux Platform Boundary

Prefer direct `/proc`, `/sys`, netlink, ioctl, sockets, and D-Bus interfaces appropriate to the metric source. eBPF/libbpf is a platform implementation detail used when the required semantic cannot be obtained cheaply and correctly from simpler stable interfaces.

## Collection ownership

Each metric collector owns its OS interaction, parsing, source-specific mutable state, and translation into core-owned types. The Linux backend composes collectors into `RawSnapshot` and reconciles peer sources where one metric spans multiple native facilities.

Linux GPU collection merges AMD `amdgpu` DRM/sysfs telemetry with NVIDIA NVML. Generic hwmon discovery emits temperature observations with canonical physical-device identity where available; it does not encode vendor ownership exclusions. Backend reconciliation consumes a matching hwmon temperature into an actually discovered GPU snapshot, leaves unmatched sensors generic, and prefers an existing vendor GPU temperature over the fallback.

The NVML adapter dynamically loads `libnvidia-ml.so.1`, owns one reusable session/device set, and exposes safe source values and metadata to the GPU collector. Missing libraries, initialization failure, zero devices, or unsupported fields degrade only that source/device/field. Production collection does not spawn `nvidia-smi`.

AMD collection is capability-based. DRM discovery identifies `amdgpu` devices and reads supported telemetry from available sysfs/hwmon files. Card numbers and native paths remain locators, not shared identities.

Long-lived eBPF collectors own their links, maps, and bounded native state. Disk and network attribution may share a narrow libbpf lifecycle/map-access adapter, but their attribution semantics and map schemas remain independent. Privilege, verifier, BTF, or attach failures affect only the dependent metric and remain typed failures rather than fabricated zeroes.

## Desktop integration

`cclover-desktop` is the sole authority for Linux monitor-surface hosting. It selects Wayland layer-shell or X11 at runtime and passes the shared `NativeScene` to native hosting/drawing code. Cairo executes shared scene primitives for both display protocols.

Wayland owns layer-shell anchoring, layer, exclusive-zone, and empty input-region behavior. X11 owns EWMH/window-manager semantics and X Shape input passthrough. Shared placement values such as panel margin have one Linux desktop authority. StatusNotifierItem tray integration is shared across X11 and Wayland and communicates only platform-neutral lifecycle intent upward.

## Test and diagnostic seams

Text/binary OS parsing stays separable from native I/O so parsers can be exercised with fixtures. Filesystem-backed discovery and identity policy expose only the narrow path/root seam needed to test real filesystem behavior against temporary trees and symlinks; this seam does not replace collection policy with a generic filesystem abstraction.

`ProbeKind` is the metadata authority for development collector names, aliases, and probe-only follow-up-sampling policy. `probe` and `perf collector` share one dispatch to production collectors. Program behavior consumes typed collection state, never diagnostic text.
