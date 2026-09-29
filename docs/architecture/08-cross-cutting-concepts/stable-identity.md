---
summary: "Defines stable semantic identity and native-locator canonicalization across platform sources."
viewpoint: static
concerns:
  - architecture-coherence
  - portability
  - maintainability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - core
    - platform
---

# Stable Identity

Cross-sample delta, history, joins, caches, and deduplication use explicit core-owned semantic identities. Native IDs, enumeration order, device names, display labels, and other reusable-looking fields remain locators or presentation data unless their lifetime semantics are explicitly part of the platform contract.

A platform canonicalizes every native source that describes the same entity into the same core identity before crossing the boundary. Platform-specific representation and units remain private.

## Processes

`ProcessInstanceId` identifies a process instance, not merely a PID.

- Linux uses PID plus `/proc/<pid>/stat` field 22 (`starttime`) as the canonical birth-marker domain. Native sources such as eBPF `task_struct::start_boottime` normalize into that same clock-tick domain before constructing the identity.
- Windows uses PID plus the process creation timestamp returned in the same `SystemProcessInformation` snapshot.

PID reuse therefore creates a new identity and never inherits prior CPU or I/O counters.

## Devices

`GpuId`, `NetworkId`, and `DiskId` are opaque core-owned identities. Native locators such as NVML UUIDs, ADL UDIDs/PCI positions, interface indices, sysfs paths, device numbers, handles, or `PhysicalDriveN` remain platform-private.

Linux network identity is derived from canonical sysfs device identity plus interface index so rename does not change `NetworkId`; disk identity uses canonical sysfs device identity plus device number so the kernel block name remains a label.

Windows network identity prefers `InterfaceGuid`. Windows disk identity prefers stable serial/device descriptors; `PhysicalDriveN` may be used only as a session-local fallback, which makes the observation degraded rather than reboot-stable.
