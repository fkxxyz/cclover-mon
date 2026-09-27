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

## Linux

Prefer direct `/proc`, `/sys`, netlink, ioctl, sockets, and D-Bus interfaces according to the metric source.

eBPF/libbpf is an allowed Linux-native source when the required semantic cannot be obtained cheaply and correctly from simpler stable interfaces. eBPF is a platform implementation detail, not a core abstraction. Build-time BPF objects, libbpf handles, kernel structs, BPF map descriptors, `dev_t`, and `ifindex` stay inside the Linux platform boundary.

Linux desktop integration is a platform responsibility separate from shared Iced drawing. It selects Wayland layer-shell or X11 at runtime from the available display environment. Wayland owns layer-shell anchoring/layer/exclusive-zone and input-region behavior; X11 owns X11/EWMH window-manager semantics and X11 input-shape behavior. The monitor surface must expose an empty pointer-input region so clicks and pointer interaction pass through to surfaces beneath it. On X11 this is enforced through the X server input shape rather than window-manager hints. X11 requests `SKIP_TASKBAR`, `SKIP_PAGER`, and `BELOW` through standard `_NET_WM_STATE` client messages after mapping, while also publishing the corresponding property values as a compatibility hint; do not branch on individual window-manager names. Neither protocol may leak into `ui`, `presentation`, or core metric types.

Linux system-tray integration uses StatusNotifierItem over the desktop session D-Bus and is independent of X11-versus-Wayland monitor hosting. Tray callbacks translate native menu activation into platform-neutral desktop commands; they do not call `process::exit`, manipulate Iced widgets, or expose D-Bus types outside the platform boundary. Failure to register the tray is a degradable desktop-integration failure: emit a diagnostic and keep the monitor running.

Partition native collection by metric responsibility. Each metric collector owns its OS interaction, parsing, and metric-specific mutable state. The Linux `Backend` composes those collectors into the core-owned `RawSnapshot`; it does not own metric-specific collection algorithms or state.

Long-lived event-driven collectors own their attach/detach lifecycle and bounded native state. Disk and network eBPF attribution may reuse a small userspace loader/map-access layer, but shared infrastructure must not merge their distinct attribution semantics into one generic kernel-hook abstraction.

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
