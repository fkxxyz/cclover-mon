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

Active — P2.

## Problem

Text parsers are often separable and unit-tested, but complete Linux discovery/IO paths remain hard-wired to live `/proc` and `/sys` locations. Important behavior around enumeration, filtering, symlinks, partial failures, and hotplug therefore lacks cheap deterministic fixture coverage.

## Evidence

CPU/memory read fixed procfs paths; process collection enumerates `/proc`; network/disk/hwmon discovery enumerates fixed sysfs roots; device identity and eBPF native-ID resolution also canonicalize live sysfs paths. Existing unit tests mainly exercise parser helpers and small pure functions.

## Maintenance impact

Collector discovery changes require host-specific runtime testing and are difficult to reproduce in CI. Verification cost encourages gaps exactly where heterogeneous machines produce the most variation.

## Governing constraint

Native IO may remain platform-specific, but interpretation and discovery policy should be testable against representative deterministic inputs without requiring the developer machine to expose the target hardware/topology.

## Resolution direction

Introduce the smallest useful filesystem/native-IO seam or fixture-root mechanism around discovery. Avoid broad mock-heavy interfaces when a path/root or data-source boundary is sufficient.

## Exit criteria

Representative success, absence, malformed data, permission/error, and multi-device discovery cases can be tested deterministically for major filesystem-backed collectors without relying on the live host.
