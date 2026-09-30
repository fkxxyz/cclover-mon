---
summary: "Keeps disk I/O accounting on physical devices while allowing platform-provided user-recognizable labels such as Windows drive letters for presentation."
viewpoint: decision
concerns:
  - architecture-coherence
  - maintainability
  - portability
  - performance
activities:
  - orient
  - change
  - assess
facets:
  area:
    - core
    - platform
    - ui
    - whole-system
---

# ADR 011: Physical-Disk Accounting with Display Aliases

## Decision

Disk I/O remains a **physical-device metric on every platform**. `DiskId` identifies the physical storage device, and disk rate/history derivation is keyed only by that identity. Logical volumes, mount points, Windows drive letters, hardware model strings, and other display-oriented names never define disk identity or accounting scope.

A physical disk carries structured presentation metadata separately from identity and counters:

- `system_label` is a short OS-level label for the physical device, such as `nvme0n1` on Linux or `Disk 0` on Windows.
- `associated_labels` are user-recognizable logical-storage labels associated with that physical disk. They describe storage topology only; they do not mean that the physical-device I/O counter belongs exclusively to any listed logical volume.

Presentation owns the display policy. When associated labels exist, the dashboard prefers them over the technical system label; otherwise it falls back to `system_label`. Renderers receive that renderer-neutral presentation result and do not infer platform-specific naming rules.

## Windows Mapping

Windows continues to sample physical I/O from `PhysicalDriveN` using `IOCTL_DISK_PERFORMANCE`. Separately, the platform collector enumerates logical drive letters and resolves each volume with `IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS`. The returned disk numbers are platform-private join keys used to associate drive letters with the corresponding physical observations.

The topology relation is many-to-many. One physical disk may carry several drive letters, and one logical volume may span several physical disks. A spanning volume therefore contributes its label to every physical disk reported by its extents; its I/O is not redistributed or re-accounted as logical-volume I/O.

Drive-letter discovery is auxiliary metadata. Failure to obtain a drive binding does not make an otherwise valid physical disk metric unavailable or degraded; presentation falls back to the physical disk's `system_label`.

## Linux Mapping

Linux keeps the existing physical-block-device semantic. The kernel block-device name is used as `system_label`, while `associated_labels` is currently empty. Mount points and partitions are not introduced as disk metric identities merely to mirror the Windows presentation mechanism.

## Rationale

Users on Windows commonly recognize storage by `C:`, `D:`, and similar drive letters, while raw hardware product strings are often long and operationally meaningless to non-expert users. Replacing physical-device accounting with logical-volume accounting would solve the naming problem by changing the metric itself, breaking cross-platform semantic consistency and introducing different behavior for spanning or composite storage.

Separating identity, accounting scope, and presentation metadata preserves one metric contract while allowing each platform to expose the most recognizable labels it can provide. A drive-letter change therefore updates presentation without resetting rate derivation or history.

## Consequences

The core disk model carries structured metadata rather than a single overloaded `name`. Platform collectors provide facts; presentation formats those facts. Windows may incur a small additional topology query during disk collection; caching is intentionally not introduced until measurement shows that the query is material to the sampling budget.

Any future proposal to account disk I/O by partition, volume, mount point, or filesystem requires a distinct metric semantic rather than silently redefining the existing disk metric.
