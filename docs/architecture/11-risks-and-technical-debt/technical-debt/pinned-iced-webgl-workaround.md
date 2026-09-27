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

Iced 0.14 and current upstream development revisions can corrupt multi-Canvas rendering on the WebGL path: geometry may be lost when Canvas widgets share a renderer layer, while naïvely forcing separate layers can still produce cross-Canvas clipping/coordinate corruption. The shared HTTP panel therefore cannot rely on the unmodified upstream `iced_widget` package.

## Current containment

`Cargo.toml` patches only `iced_widget` to a fixed commit in `fkxxyz/iced`. The fork keeps each Canvas inside its own renderer layer and creates that layer after entering the Canvas-local translation, so clipping and geometry share one coordinate transform. The existing Canvas API and shared native/Web panel implementation remain unchanged; other Iced crates stay on their normal crates.io versions.

## Maintenance impact

Dependency upgrades must verify whether the upstream defect still exists and whether the local patch still applies. The fork is an additional source dependency that must remain reachable and reproducible.

## Governing constraint

Keep the workaround below the shared UI boundary. Do not introduce Web-only graph widgets, duplicate panel layout, or merge all application graphs into one Canvas merely to accommodate this renderer defect.

## Exit criteria

Remove the fork pin when an upstream Iced release fixes the multi-Canvas WebGL behavior and the real HTTP panel is runtime-validated with simultaneous MEMORY, CPU, temperature, disk, and both network graphs rendering correctly.
