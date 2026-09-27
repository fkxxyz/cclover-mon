---
summary: "Tracks mixed shared libbpf runtime and metric-specific attribution responsibilities in one userspace module."
viewpoint: assurance
concerns:
  - maintainability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
---

# eBPF Userspace Responsibility Concentration

## Status

Active — P2.

## Problem

Linux eBPF userspace support combines generic libbpf FFI/object/map lifecycle with disk attribution semantics and network attribution semantics in one `ebpf_io` module and one aggregate collector.

## Evidence

The module owns raw libbpf declarations, object/link lifecycle, generic map iteration, disk key decoding and device resolution, network key decoding and interface resolution, row merging, and failure mapping.

## Maintenance impact

Changes to one attribution mechanism increase review surface and coupling with unrelated attribution logic. Growth in hooks, caches, native-ID resolution, diagnostics, or additional eBPF metrics can recreate a backend-sized responsibility aggregate.

## Governing constraint

Only genuinely generic libbpf runtime/map-access concerns should be shared. Disk and network attribution semantics must remain independently owned even if they reuse the same low-level runtime.

## Resolution direction

Separate a narrow shared eBPF runtime/RAII layer from disk-attribution and network-attribution modules. Avoid a generic attribution abstraction that forces distinct kernel semantics into one model.

## Exit criteria

Disk or network attribution can evolve without modifying the other metric's semantic module, while common libbpf lifecycle/map operations remain shared once.
