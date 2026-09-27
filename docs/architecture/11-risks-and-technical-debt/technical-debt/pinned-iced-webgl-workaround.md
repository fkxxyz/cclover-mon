---
summary: "Tracks the temporary iced_widget fork required for correct multi-Canvas WebGL rendering."
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

# Pinned Iced WebGL Workaround

## Status

Active — P3.

## Problem

Iced 0.14 and current upstream development revisions can render only the final `Canvas` geometry when multiple Canvas widgets are merged into one renderer layer on the WebGL path. The shared HTTP panel therefore cannot rely on the unmodified upstream `iced_widget` package.

## Current containment

`Cargo.toml` patches only `iced_widget` to a fixed commit in `fkxxyz/iced`. The fork keeps each Canvas inside its own renderer layer while preserving the existing Canvas API and the shared native/Web panel implementation. Other Iced crates remain on their normal crates.io versions.

## Maintenance impact

Dependency upgrades must verify whether the upstream defect still exists and whether the local patch still applies. The fork is an additional source dependency that must remain reachable and reproducible.

## Governing constraint

Keep the workaround below the shared UI boundary. Do not introduce Web-only graph widgets, duplicate panel layout, or merge all application graphs into one Canvas merely to accommodate this renderer defect.

## Exit criteria

Remove the fork pin when an upstream Iced release fixes the multi-Canvas WebGL behavior and the real HTTP panel is runtime-validated with simultaneous MEMORY, CPU, temperature, disk, and both network graphs rendering correctly.
