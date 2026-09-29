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

## Problem

Iced 0.14 and current upstream development revisions can corrupt multi-Canvas rendering on the WebGL path: geometry may be lost when Canvas widgets share a renderer layer, while naïvely forcing separate layers can still produce cross-Canvas clipping/coordinate corruption. The Web/WASM renderer adapter for the HTTP panel therefore cannot rely on the unmodified upstream `iced_widget` package.

## Current containment

`Cargo.toml` patches only `iced_widget` to a fixed commit in `fkxxyz/iced`. The fork keeps each Canvas inside its own renderer layer and creates that layer after entering the Canvas-local translation, so clipping and geometry share one coordinate transform. Iced is now confined to `cclover-web-ui`; native desktop rendering uses the shared dashboard definition through `NativeScene` and platform-native renderers, so this fork no longer affects Linux or Windows desktop rendering. Other Iced crates stay on their normal crates.io versions.

## Maintenance impact

Dependency upgrades must verify whether the upstream defect still exists and whether the local patch still applies. The fork is an additional source dependency that must remain reachable and reproducible.

## Governing constraint

Keep the workaround inside the Web/WASM renderer adapter and below the shared dashboard-definition boundary. Do not duplicate dashboard semantics/layout or merge all application graphs into one Canvas merely to accommodate this renderer defect.

## Exit criteria

Remove the fork pin when an upstream Iced release fixes the multi-Canvas WebGL behavior and the real HTTP panel is runtime-validated with simultaneous MEMORY, CPU, temperature, disk, and both network graphs rendering correctly.
