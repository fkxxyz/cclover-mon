---
summary: "Records the remaining dual-authority debt for Windows AMD Family 10h temperature decoding."
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

# Windows AMD Family 10h Upstream Authority

**Priority:** Low-medium

## Root cause

Windows AMD Family 10h temperature collection cannot currently execute the selected upstream Linux `k10temp` path through the pinned official PawnIO transport. Linux `k10temp` performs Erratum 319 validation using northbridge function-2 PCI configuration reads, while the official signed `AMDFamily10.bin` capability used by cclover-mon exposes only the narrower function-3 temperature/configuration reads required by the existing implementation.

## Evidence

Windows AMD Family 11h and newer processors use the vendored unchanged Linux `k10temp.c` compatibility path, while Family 10h retains the older project-owned LibreHardwareMonitor-derived temperature decode. The pinned PawnIO `AMDFamily10.bin` module does not expose the function-2 PCI configuration read required by upstream `has_erratum_319()`, and the currently pinned official module set has no alternative signed capability that supplies that access within the architecture boundary.

## Governing constraint

For hardware families reachable through supported read-only Windows transports, low-level temperature algorithms should have one primary upstream authority: the selected unchanged Linux hwmon source. The project must not bypass upstream hardware-safety checks or replace pinned official signed PawnIO modules with locally rebuilt privileged modules merely to achieve source unification.

## Scope discovery

Review Windows AMD CPU-temperature routing for Family 10h, the `k10temp` compatibility bridge, PawnIO `AMDFamily10` transport capabilities, and any future official signed module capability that permits the exact PCI configuration reads needed by upstream Erratum 319 handling. Include any project-owned Family 10h temperature decode that remains after transport support changes.

## Maintenance consequence

Family 10h temperature behavior has a second algorithm authority that must be reviewed independently from Linux `k10temp`. Upstream fixes or semantic changes for this family are not inherited automatically, and maintainers must remember that "AMD uses k10temp" has one legacy exception.

## Repair direction

When a pinned official signed PawnIO capability can provide the required function-2 PCI configuration reads through a narrow read-only transport, route Family 10h through the existing `k10temp` compatibility path and remove the project-owned Family 10h decode. Preserve upstream Erratum 319 handling rather than reproducing or bypassing it in project code.

## Exit criteria

- Windows AMD Family 10h temperature collection executes unchanged vendored `k10temp.c` for its hardware algorithm.
- Upstream Erratum 319 handling runs through a narrow supported Windows transport rather than being skipped or reimplemented.
- No project-owned Family 10h temperature decode remains.
- Only pinned official signed PawnIO modules are required at runtime.
- Deterministic validation and representative hardware validation, when hardware is available, confirm unchanged public temperature identity and availability semantics.
