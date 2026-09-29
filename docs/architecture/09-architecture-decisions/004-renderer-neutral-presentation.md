---
summary: "Records the renderer-neutral presentation boundary; ADR 008 adds shared graphical dashboard layout above it."
viewpoint: decision
concerns:
  - portability
  - maintainability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
---

# ADR 004: Renderer-Neutral Presentation Model

## Status

Presentation ownership remains current. ADR 008 supersedes only the former rule that each graphical frontend independently owns layout and that native/Web must share Iced pixel rendering.

## Decision

Introduce a renderer-neutral presentation layer between `core::MonitorState` and concrete frontends.

The presentation layer owns dashboard semantics that should remain consistent across graphical renderers and the terminal frontend: metric grouping, display labels, value formatting, unavailable-value semantics, process rows, and access to the history associated with a displayed metric.

ADR 008 now assigns graphical dashboard structure and shared geometry to `cclover-ui`; concrete renderers own only realization mechanics. The terminal frontend owns terminal rows, columns, focus, scrolling, and terminal rendering. Do not introduce a generic renderer/widget trait merely to make these layout systems look alike.

Graphical structural layout has one authority in `cclover-ui`, including requested native panel height. Renderer adapters must not maintain a second hand-written copy of panel structure.

## Rationale

Linux desktop, Windows desktop, and the terminal interface share metric meaning but not rendering mechanics. Sharing at the presentation boundary preserves consistent semantics without coupling terminal behavior to Iced or forcing desktop layout through a lowest-common-denominator renderer abstraction.

Keeping the presentation model renderer-neutral also preserves the existing native typed data flow and avoids serialization, IPC, or duplicate derivation of rates, history, and Top-N process data.

## Consequences

- `core` remains the authority for metric semantics, derivation, history, and aggregation.
- `presentation` may depend on core model types and shared formatting rules, but not on Iced, terminal libraries, platform APIs, or app message types.
- Historical rendering consequence superseded by ADR 008: graphical targets now share `cclover-ui` dashboard authority while renderer mechanics may differ.
- The terminal frontend reuses `presentation` while defining its own layout and interaction; sampling and platform composition remain in the execution adapter.
- Frontend layout metadata is renderer-specific and must not leak into the presentation model.
