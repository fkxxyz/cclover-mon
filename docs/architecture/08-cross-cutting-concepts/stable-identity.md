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

`GpuId`, `NetworkId`, `DiskId`, and hardware-sensor identities such as `FanId` are opaque core-owned identities. Native locators such as NVML UUIDs, ADL UDIDs/PCI positions, Super-I/O slots/channels, interface indices, sysfs paths, device numbers, handles, or `PhysicalDriveN` remain platform-private.

Hardware-sensor display labels are not identities. Windows canonicalizes a stable physical source/chip/channel identity before applying optional board-specific names, so learning that `Fan #1` is `CPU Fan` does not create a new fan. Linux hwmon likewise derives identity from the hardware source/channel rather than the volatile `hwmonN` enumeration directory or label text.

Linux network identity is derived from canonical sysfs device identity plus interface index so rename does not change `NetworkId`; disk identity uses canonical sysfs device identity plus device number so the kernel block name remains a label.

Windows network identity prefers `InterfaceGuid`. Windows disk identity prefers stable serial/device descriptors; `PhysicalDriveN` may be used only as a session-local fallback, which makes the observation degraded rather than reboot-stable.

Disk display metadata is deliberately separate from this identity. A physical disk may carry an OS-level `system_label` plus zero or more `associated_labels`; Windows drive letters are topology-derived associated labels, not disk identities and not I/O accounting scopes. Changing `D:` to `E:` therefore does not create a new disk or reset rate/history state. See [ADR 011](../09-architecture-decisions/011-physical-disk-display-aliases.md).
