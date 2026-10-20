---
summary: "Records that Linux physical-device selection is not one authority across collectors and per-process attribution, and rests on synthetic fixtures from a single device class."
viewpoint: assurance
concerns:
  - maintainability
  - portability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - platform
    - whole-system
---

# Linux Counted-Device Selection

**Priority:** Low-medium

## Root cause

The decision whether a block device or network interface is a counted device is implemented inside the disk and network collectors. The eBPF per-process attribution path resolves identities through separate entry points that do not apply it, and the rule's evidence is a synthetic fixture model plus one live device class rather than captured kernel output.

## Evidence

Measured on one aarch64 OpenWrt router: the collectors exclude eMMC boot partitions, MTD, UBI, and FIT block devices and the DSA conduit interface. Established by reading the code, not by runtime test: the attribution resolvers (`id_for_name_from`, `id_for_interface_from`) return an identity for those same devices, and the disk resolver additionally falls back to the sysfs path when a device has no `device` link, which the collector skips. Process rows carrying such identities are grouped by identity and matched to panels by identity, so rows for uncounted devices stay in the snapshot but appear in no panel.

A direct fix is not safe. Making the resolvers return no identity for uncounted devices increments `unresolved_native_ids`, which marks the whole attribution collection degraded; a benign write to an excluded device would then show as a fault. The fixtures in the collector tests are built from the author's model of sysfs; only the live router run validates the `platform` bus rule.

## Governing constraint

One predicate decides whether a device is counted, and every path that produces or resolves a device identity uses it. Attribution to an uncounted device is dropped as an intentional outcome that is distinct from an unresolvable identity and does not degrade the collection. Rules derived from device behavior are backed by captured evidence from representative devices.

## Scope discovery

Review `linux/disk.rs`, `linux/network.rs`, and the resolvers in `linux/ebpf_io/disk.rs` and `linux/ebpf_io/network.rs`, including the no-`device`-link fallback. Include how `ProcessDiskIo` and `ProcessNetworkIo` rows are carried to the API and presentation. Include every fixture that models sysfs layout. Exclude Windows collectors and attribution on non-DSA, non-embedded hosts unless the shared predicate changes their results.

## Maintenance consequence

Each new exclusion rule must be applied in several places, and an omission is silent. A deployment that enables eBPF on a device with excluded devices carries orphan process rows. A rule that is correct on one device class can hide or keep the wrong device on another with no failing test.

## Repair direction

Extract a three-valued selection result (counted, excluded with reason, not a device) shared by the collectors and resolvers, and make the attribution collectors drop excluded identities without degrading. Add a small corpus of captured, anonymized sysfs trees from representative devices (eMMC with boot partitions, MTD/UBI flash, FIT, NVMe, DSA switch) with device class and kernel version as provenance, and replay it through the collectors.

## Exit criteria

- Collectors and attribution resolvers call one selection function; no path decides device eligibility independently.
- Attribution to an excluded device produces no row and does not mark the collection degraded.
- A checked-in corpus of captured sysfs trees with provenance drives the selection tests.
- A runtime check with eBPF enabled on a device that has excluded devices shows no orphan process rows.
