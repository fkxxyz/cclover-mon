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

Platform backends implement the collection contracts owned by `core` and translate native state into core-owned, platform-neutral snapshot types. `core` never imports a platform backend; the application composition root wires the selected backend into the core sampler. Native desktop startup enters the shared `cclover-desktop` host contract on every supported desktop OS; application code supplies lifecycle behavior and desired surface geometry, while each platform host owns native window/runtime realization.

Rust source is safe by default: the crate denies `unsafe_code`. Raw FFI, pointer manipulation, dynamic symbol loading, and native lifetime mechanics may opt out only inside narrowly scoped native adapter modules. Every unsafe block or unsafe trait implementation in those adapters documents its local `SAFETY` invariant. Metric policy, shaping, availability semantics, and cross-sample logic stay on the safe side of that boundary.

Stable identity is translated at this boundary as well. Native locators or timestamps may be used to construct a core-owned identity, but their platform-specific representation and units must not leak into shared code. A platform must canonicalize all native sources that describe the same entity into the same core identity before crossing the boundary; otherwise cross-source joins would create conflicting identities for one entity.

## Linux

Prefer direct `/proc`, `/sys`, netlink, ioctl, sockets, and D-Bus interfaces according to the metric source.

Linux process identity uses PID plus a process birth marker. `/proc/<pid>/stat` field 22 (`starttime`) is the canonical Linux birth marker domain. Native sources that expose the same kernel birth time in another unit, including eBPF `task_struct::start_boottime`, must normalize it to the same clock-tick domain before constructing `ProcessInstanceId`. PID by itself is never a cross-sample process identity.

Linux physical network and block collectors similarly translate native identity before returning shared counters. Network identity is derived from the canonical sysfs device target plus the interface index so an interface rename does not change `NetworkId`; disk identity is derived from the canonical sysfs device target plus its device number so the kernel block name remains only a display label. The resulting opaque `NetworkId` / `DiskId` values are the only keys used for core delta and history association. `ifindex`, `dev_t`/device-number syntax, and sysfs paths remain Linux implementation details and must not be interpreted by core, presentation, or frontend code.

eBPF/libbpf is an allowed Linux-native source when the required semantic cannot be obtained cheaply and correctly from simpler stable interfaces. eBPF is a platform implementation detail, not a core abstraction. Build-time BPF objects, libbpf handles, kernel structs, BPF map descriptors, `dev_t`, and `ifindex` stay inside the Linux platform boundary.

Linux desktop integration in `cclover-desktop` is the sole authority for native monitor-surface hosting, separate from shared Iced drawing and metric collection. The application supplies platform-neutral lifecycle behavior and desired surface geometry; the desktop host selects Wayland layer-shell or X11 at runtime and translates those requests into the active protocol. `main`, `app`, `ui`, `presentation`, and core metric types must not select a display server, construct protocol-specific window settings, emit layer-shell actions, or perform X11 resize/configuration operations. Wayland owns layer-shell anchoring/layer/exclusive-zone and input-region behavior; X11 owns X11/EWMH window-manager semantics and X11 input-shape behavior. The monitor surface must expose an empty pointer-input region so clicks and pointer interaction pass through to surfaces beneath it. On X11 this is enforced through the X server input shape rather than window-manager hints. X11 requests `SKIP_TASKBAR`, `SKIP_PAGER`, and `BELOW` through standard `_NET_WM_STATE` client messages after mapping, while also publishing the corresponding property values as a compatibility hint; do not branch on individual window-manager names.

Desktop protocol configuration keeps host orchestration separate from protocol policy. The platform-owned host may adapt platform-neutral application messages, subscriptions, theme, and desired surface size into the selected runtime, but application update code must remain unaware of how a native surface is resized or configured. Entry points may coordinate connection/setup and final flush, while independently meaningful policies such as input shape, window-manager state, workspace visibility, placement, and client-message requests belong in named helpers. New protocol behavior should extend the corresponding policy helper rather than enlarge one monolithic configuration routine. Placement constants shared by Wayland and X11, such as the panel margin, have one Linux desktop authority rather than parallel protocol-specific copies.

Linux system-tray integration uses StatusNotifierItem over the desktop session D-Bus and is independent of X11-versus-Wayland monitor hosting. Tray callbacks translate native menu activation into platform-neutral desktop commands; they do not call `process::exit`, manipulate Iced widgets, or expose D-Bus types outside the platform boundary. Failure to register the tray is a degradable desktop-integration failure: emit a diagnostic and keep the monitor running.

Partition native collection by metric responsibility. Each metric collector owns its OS interaction, parsing, and metric-specific mutable state. The Linux `Backend` composes those collectors into the core-owned `RawSnapshot`; it does not own metric-specific collection algorithms or state. Every raw metric crosses this boundary as the core-owned `Collection<T>` outcome: `Available(T)` means a complete observation, `Degraded(T)` means a usable partial observation, and `Unavailable(reason)` means no observation. An empty collection payload is therefore a valid observed value, never an implicit failure signal.

One metric collector may own multiple peer native sources when the OS exposes the same semantic through different facilities. Linux temperature collection uses hwmon as the generic kernel sensor source and an NVIDIA NVML source for proprietary-driver GPUs. The temperature collector merges both into the same core-owned temperature snapshot sequence. `hwmon`, NVML handles, NVIDIA UUID/PCI APIs, and source-specific availability states do not cross into core, presentation, or UI.

The NVML source is loaded dynamically at runtime rather than linked as a mandatory process dependency. Missing `libnvidia-ml.so.1`, NVML initialization failure, zero discovered NVIDIA devices, or an individual device lacking a readable temperature are degradable source conditions: omit the unavailable NVIDIA temperature entries and continue collecting all unrelated temperatures. Production collection must call NVML in-process; it must not spawn `nvidia-smi` or another helper process. Enumerate all NVML devices, retain stable per-device identity, and keep reusable NVML state/handles for repeated sampling rather than rediscovering devices every cycle.

NVML dynamic-loader handles, function pointers, device handles, C strings, and shutdown ordering belong to the NVML runtime adapter. The temperature collector consumes only its safe session/device API and owns the policy for diagnostics and translation into `TemperatureSnapshot`.

Long-lived event-driven collectors own their attach/detach lifecycle and bounded native state. Disk and network eBPF attribution may reuse a small userspace loader/map-access layer, but shared infrastructure must not merge their distinct attribution semantics into one generic kernel-hook abstraction.

The libbpf runtime adapter owns raw libbpf object/link/map handles and the unsafe map-access calls. Disk and network attribution collectors consume safe loader/map operations and keep attribution semantics outside that adapter. Small direct libc queries used by otherwise-safe Linux collectors likewise pass through the dedicated Linux native helper rather than introducing local unsafe blocks.

Any BPF map key or value whose bytes are read directly into a Rust type is a cross-language ABI and must be mechanically layout-verified at build time. The Rust type uses `#[repr(C)]`; the build derives `sizeof`, alignment, and every field offset from clang's actual BPF-target C record layout and compilation fails if the Rust layout differs. Adding or changing a userspace-consumed BPF map schema is incomplete until the corresponding layout verification covers its full key/value structure. Manual comparison of C and Rust declarations is not an acceptable compatibility mechanism.

Keep parsing of textual or binary OS formats separable from native IO so representative fixtures can exercise parsers without relying on the developer machine's live `/proc` or `/sys` contents. Development probes must invoke the same production collector path used by normal sampling rather than maintain a parallel collection implementation.

Development collector identity has one metadata authority: `ProbeKind` owns canonical CLI names, accepted aliases, and probe-only follow-up-sampling policy. CLI parsing and help derive from that metadata rather than maintaining parallel collector lists. Within each platform backend, `probe` and `perf collector` share one `ProbeKind` → production-collector dispatch; they may consume the resulting typed sample differently, but must not duplicate collector selection. Normal batch snapshot composition remains explicit because its `RawSnapshot` fields are the production sampling contract, not a second development-command registry.

Privileged Linux sources use least authority. Permission, verifier, BTF, or attach failures are surfaced as typed unavailability/diagnostics for the affected metric rather than converted to zero or causing unrelated collectors to fail. Program decisions consume only typed collection state; diagnostic text is human-readable evidence and must never be parsed to infer availability or degradation. When a delta-based source is unavailable, that sample cannot serve as a comparison baseline; the next observable sample starts a new zero-rate baseline rather than spanning the unavailable interval.

## Windows

Prefer direct native APIs that expose the required cumulative counters without subprocesses or WMI polling. The Windows collector source mapping is explicit:

| Metric responsibility | Native source |
| --- | --- |
| aggregate CPU | `GetSystemTimes`; `GetActiveProcessorCount(ALL_PROCESSOR_GROUPS)` supplies the logical-CPU scale used by process CPU derivation |
| physical memory | `GlobalMemoryStatusEx` |
| processes | `NtQuerySystemInformation(SystemProcessInformation)` as one batch snapshot; creation time is the Windows `ProcessInstanceId` birth marker, user+kernel time is the cumulative CPU counter, and working-set size is the memory counter |
| network interfaces | IP Helper `GetIfTable2`; `InterfaceGuid` becomes `NetworkId`, `Alias` remains display text, and `InOctets` / `OutOctets` remain cumulative counters for core rate derivation |
| physical disks | `CreateFile(\\.\\PhysicalDriveN)` plus `DeviceIoControl(IOCTL_DISK_PERFORMANCE)`; Storage property queries provide serial/product identity where available before translation to `DiskId` |
| temperatures | PawnIO is the preferred future low-level CPU/Super-I/O access layer; GPU vendor APIs and storage SMART/NVMe health remain peer temperature sources |
| per-process disk/network attribution | separate future event-attribution work, expected to use ETW/WFP-class facilities rather than PawnIO |

Do not route these basic collectors through PDH when a direct structured API above already owns the semantic. Process/NT-object batch telemetry preferentially uses the NT query API rather than opening every process individually. PawnIO is a hardware-register access dependency, not a process-I/O attribution mechanism.

Windows process identity is PID plus the process creation timestamp returned in the same `SystemProcessInformation` snapshot. PID alone is never a cross-sample identity. Network interface index and `PhysicalDriveN` enumeration number are locators, not identities; prefer `InterfaceGuid` and storage serial/device descriptors respectively.

Windows physical-network selection uses the `MIB_IF_ROW2` hardware-interface capability rather than adapter-name patterns.

Windows capabilities that are not implemented yet remain explicit typed unavailability. Temperature collection and per-process disk/network attribution return `Unavailable(Unsupported)` until their native sources exist; they must not be represented as empty observations or fabricated zero counters.

Windows disk identity prefers storage serial/device descriptors. If no stable serial is available, the collector may fall back to `PhysicalDriveN` only as a session-local locator and must mark the disk collection `Degraded`; that fallback must not be treated as reboot-stable identity by core, presentation, or frontend code.

Windows monitor-surface creation and resize realization use the same platform-owned desktop-host contract as Linux; `main` and shared application update code do not construct native windows directly. The Windows host owns monitor placement and shell policy: the monitor surface is undecorated, non-resizable, excluded from the taskbar and Alt+Tab, non-activating, placed at the top-right monitor margin, kept below normal windows, and configured for mouse passthrough. Iced/winit window settings express portable policy, while the Windows native adapter applies `WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE` and `SetWindowPos(HWND_BOTTOM, ..., SWP_NOACTIVATE)` to enforce shell/Z-order semantics that generic window settings do not guarantee. Dynamic panel resize must reapply the top-right placement and bottom Z-order. Future Windows notification-area integration belongs behind the same desktop-integration boundary. Native shell handles and menu identifiers remain platform-private, while user intent is translated into the same platform-neutral desktop commands consumed by the application lifecycle.

Windows desktop and Web text must not depend on a font being installed by the host environment. Those targets use Iced's bundled Fira Sans font; Linux keeps its existing Inconsolata choice.

## C++ interoperability

```text
Rust → C ABI → thin C++ bridge → C++ library / SDK
```

Exchange POD data, buffers, opaque handles, status codes, and callbacks across the ABI boundary. C++ library types remain behind the bridge.
