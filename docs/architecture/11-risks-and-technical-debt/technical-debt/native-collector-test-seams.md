---
summary: "Tracks native collector discovery and IO paths that are difficult to validate with deterministic fixtures."
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
    - whole-system
---

# Native Collector Test Seams

## Status

Resolved.

## Problem

Text parsers are often separable and unit-tested, but complete Linux discovery/IO paths remain hard-wired to live `/proc` and `/sys` locations. Important behavior around enumeration, filtering, symlinks, partial failures, and hotplug therefore lacks cheap deterministic fixture coverage.

## Evidence

CPU/memory read fixed procfs paths; process collection enumerates `/proc`; network/disk/hwmon discovery enumerates fixed sysfs roots; device identity and eBPF native-ID resolution also canonicalize live sysfs paths. Existing unit tests mainly exercise parser helpers and small pure functions.

## Maintenance impact

Collector discovery changes require host-specific runtime testing and are difficult to reproduce in CI. Verification cost encourages gaps exactly where heterogeneous machines produce the most variation.

## Governing constraint

Native IO may remain platform-specific, but interpretation and discovery policy should be testable against representative deterministic inputs without requiring the developer machine to expose the target hardware/topology.

## Resolution

Linux filesystem-backed collectors now keep fixed production paths in thin wrappers and expose narrow internal path/root seams for deterministic fixture testing. CPU, memory, process, disk, network, and hwmon collection can run against temporary filesystem trees; disk/network stable-identity policy and eBPF disk native-ID resolution use the same seam rather than live sysfs. AMD GPU collection already accepted a DRM root and remains on that pattern. Tests use real `std::fs` files and symlinks through a small test-only fixture helper, while permission mapping is tested directly from synthetic `std::io::Error` values instead of depending on host ownership or chmod behavior. No generic filesystem trait, mock layer, runtime configuration, or alternate collector path was introduced.

The durable rule is recorded in the platform-boundary View: filesystem-backed discovery and identity policy must retain a narrow `Path`/root seam without changing collection semantics, cadence, or ownership.

## Exit criteria

Representative success, absence, malformed data, permission/error, and multi-device discovery cases can be tested deterministically for major filesystem-backed collectors without relying on the live host.
