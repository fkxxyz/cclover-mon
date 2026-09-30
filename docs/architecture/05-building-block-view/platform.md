---
summary: "Defines native platform collector composition and ownership of metric-specific sources."
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
---

# Platform

A platform backend is the batch composition point. Metric-specific OS access, parsing, source fan-in, and mutable collector state belong to the corresponding metric responsibility rather than to one monolithic backend.

```text
platform backend
  ├── CPU collector
  ├── memory collector
  ├── process collector
  ├── network collector
  ├── disk collector
  ├── temperature collector
  ├── fan collector
  └── GPU collector
```

Collectors may fan in multiple native sources when they implement one platform-neutral metric. Native source identity is canonicalized before crossing into core-owned identities.

On Linux, GPU collection merges AMD `amdgpu` DRM/sysfs telemetry and NVIDIA NVML. Generic hwmon discovery emits temperature observations without vendor ownership exclusions; the backend reconciles matching physical-device identities into discovered GPU snapshots while unmatched sensors remain generic. A vendor-provided GPU temperature wins over a matching hwmon fallback. NVIDIA session/device lifetime is shared platform adapter state; metric policy stays outside that raw adapter.

On Windows, CPU uses system timing counters, physical memory uses the system memory-status API, swap/pagefile usage uses actual page-file accounting, process collection uses one NT system-process snapshot, network uses IP Helper counters, and disk uses storage/device I/O controls. Motherboard and low-level CPU hardware telemetry has one platform runtime that owns PawnIO capabilities, hardware discovery/topology, bus synchronization, and source retry state. It produces strongly typed temperature/fan projections from one coordinated observation rather than giving each metric independent hardware ownership. Selected unchanged Linux hwmon source is the primary compatibility upstream for hardware families reachable through supported Windows transports; a project-owned minimal compatibility facade and read-only transport policy keep Linux kernel mechanics out of core-owned contracts. LibreHardwareMonitor remains a secondary reviewed knowledge source rather than a runtime library. GPU collection owns independently optional, long-lived NVIDIA NVML and AMD ADL sessions and isolates failure by source/device/field.

Linux eBPF links/maps and Windows native sessions are platform implementation details. Disk and network attribution may share userspace lifecycle infrastructure where useful, but their attribution semantics remain independently owned.
