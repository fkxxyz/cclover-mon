---
summary: "Defines the cross-cutting rules for platform isolation, native API use, and Rust/C++ interoperability."
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
    - native-bridge
---

# Platform Boundary

Platform backends implement the collection contracts owned by `core` and translate native state into core-owned, platform-neutral snapshot types. `core` never imports a platform backend; the application composition root wires the selected backend into the core sampler.

Rust source is safe by default: the crate denies `unsafe_code`. Raw FFI, pointer manipulation, dynamic symbol loading, and native lifetime mechanics may opt out only inside narrowly scoped native adapter modules. Every unsafe block or unsafe trait implementation in those adapters documents its local `SAFETY` invariant. Metric policy, shaping, availability semantics, and cross-sample logic stay on the safe side of that boundary.

Stable identity is translated at this boundary as well. Native locators or timestamps may be used to construct a core-owned identity, but their platform-specific representation and units must not leak into shared code. A platform must canonicalize all native sources that describe the same entity into the same core identity before crossing the boundary; otherwise cross-source joins would create conflicting identities for one entity.

## Linux

Prefer direct `/proc`, `/sys`, netlink, ioctl, sockets, and D-Bus interfaces according to the metric source.

Linux process identity uses PID plus a process birth marker. `/proc/<pid>/stat` field 22 (`starttime`) is the canonical Linux birth marker domain. Native sources that expose the same kernel birth time in another unit, including eBPF `task_struct::start_boottime`, must normalize it to the same clock-tick domain before constructing `ProcessInstanceId`. PID by itself is never a cross-sample process identity.

Linux physical network and block collectors similarly translate native identity before returning shared counters. Network identity is derived from the canonical sysfs device target plus the interface index so an interface rename does not change `NetworkId`; disk identity is derived from the canonical sysfs device target plus its device number so the kernel block name remains only a display label. The resulting opaque `NetworkId` / `DiskId` values are the only keys used for core delta and history association. `ifindex`, `dev_t`/device-number syntax, and sysfs paths remain Linux implementation details and must not be interpreted by core, presentation, or frontend code.

eBPF/libbpf is an allowed Linux-native source when the required semantic cannot be obtained cheaply and correctly from simpler stable interfaces. eBPF is a platform implementation detail, not a core abstraction. Build-time BPF objects, libbpf handles, kernel structs, BPF map descriptors, `dev_t`, and `ifindex` stay inside the Linux platform boundary.

Linux desktop integration is a platform responsibility separate from shared Iced drawing. It selects Wayland layer-shell or X11 at runtime from the available display environment. Wayland owns layer-shell anchoring/layer/exclusive-zone and input-region behavior; X11 owns X11/EWMH window-manager semantics and X11 input-shape behavior. The monitor surface must expose an empty pointer-input region so clicks and pointer interaction pass through to surfaces beneath it. On X11 this is enforced through the X server input shape rather than window-manager hints. X11 requests `SKIP_TASKBAR`, `SKIP_PAGER`, and `BELOW` through standard `_NET_WM_STATE` client messages after mapping, while also publishing the corresponding property values as a compatibility hint; do not branch on individual window-manager names. Neither protocol may leak into `ui`, `presentation`, or core metric types.

Linux system-tray integration uses StatusNotifierItem over the desktop session D-Bus and is independent of X11-versus-Wayland monitor hosting. Tray callbacks translate native menu activation into platform-neutral desktop commands; they do not call `process::exit`, manipulate Iced widgets, or expose D-Bus types outside the platform boundary. Failure to register the tray is a degradable desktop-integration failure: emit a diagnostic and keep the monitor running.

Partition native collection by metric responsibility. Each metric collector owns its OS interaction, parsing, and metric-specific mutable state. The Linux `Backend` composes those collectors into the core-owned `RawSnapshot`; it does not own metric-specific collection algorithms or state.

One metric collector may own multiple peer native sources when the OS exposes the same semantic through different facilities. Linux temperature collection uses hwmon as the generic kernel sensor source and an NVIDIA NVML source for proprietary-driver GPUs. The temperature collector merges both into the same core-owned temperature snapshot sequence. `hwmon`, NVML handles, NVIDIA UUID/PCI APIs, and source-specific availability states do not cross into core, presentation, or UI.

The NVML source is loaded dynamically at runtime rather than linked as a mandatory process dependency. Missing `libnvidia-ml.so.1`, NVML initialization failure, zero discovered NVIDIA devices, or an individual device lacking a readable temperature are degradable source conditions: omit the unavailable NVIDIA temperature entries and continue collecting all unrelated temperatures. Production collection must call NVML in-process; it must not spawn `nvidia-smi` or another helper process. Enumerate all NVML devices, retain stable per-device identity, and keep reusable NVML state/handles for repeated sampling rather than rediscovering devices every cycle.

NVML dynamic-loader handles, function pointers, device handles, C strings, and shutdown ordering belong to the NVML runtime adapter. The temperature collector consumes only its safe session/device API and owns the policy for diagnostics and translation into `TemperatureSnapshot`.

Long-lived event-driven collectors own their attach/detach lifecycle and bounded native state. Disk and network eBPF attribution may reuse a small userspace loader/map-access layer, but shared infrastructure must not merge their distinct attribution semantics into one generic kernel-hook abstraction.

The libbpf runtime adapter owns raw libbpf object/link/map handles and the unsafe map-access calls. Disk and network attribution collectors consume safe loader/map operations and keep attribution semantics outside that adapter. Small direct libc queries used by otherwise-safe Linux collectors likewise pass through the dedicated Linux native helper rather than introducing local unsafe blocks.

Any BPF map key or value whose bytes are read directly into a Rust type is a cross-language ABI and must be mechanically layout-verified at build time. The Rust type uses `#[repr(C)]`; the build derives `sizeof`, alignment, and every field offset from clang's actual BPF-target C record layout and compilation fails if the Rust layout differs. Adding or changing a userspace-consumed BPF map schema is incomplete until the corresponding layout verification covers its full key/value structure. Manual comparison of C and Rust declarations is not an acceptable compatibility mechanism.

Keep parsing of textual or binary OS formats separable from native IO so representative fixtures can exercise parsers without relying on the developer machine's live `/proc` or `/sys` contents. Development probes must invoke the same production collector path used by normal sampling rather than maintain a parallel collection implementation.

Privileged Linux sources use least authority. Permission, verifier, BTF, or attach failures are surfaced as typed unavailability/diagnostics for the affected metric rather than converted to zero or causing unrelated collectors to fail.

## Windows

Prefer native Win32, NT APIs, PDH, ETW, IP Helper, COM, and device APIs according to the metric source.

Future Windows notification-area integration belongs behind the same desktop-integration boundary. Native shell handles and menu identifiers remain platform-private, while user intent is translated into the same platform-neutral desktop commands consumed by the application lifecycle.

## C++ interoperability

```text
Rust → C ABI → thin C++ bridge → C++ library / SDK
```

Exchange POD data, buffers, opaque handles, status codes, and callbacks across the ABI boundary. C++ library types remain behind the bridge.
