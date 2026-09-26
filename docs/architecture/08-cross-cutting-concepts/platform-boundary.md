---
summary: "Defines the cross-cutting rules for platform isolation, native API use, and Rust/C++ interoperability."
viewpoint: static
concerns:
  - architecture-coherence
  - performance
  - portability
  - maintainability
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

## Windows

Prefer native Win32, NT APIs, PDH, ETW, IP Helper, COM, and device APIs according to the metric source.

## C++ interoperability

```text
Rust → C ABI → thin C++ bridge → C++ library / SDK
```

Exchange POD data, buffers, opaque handles, status codes, and callbacks across the ABI boundary. C++ library types remain behind the bridge.
