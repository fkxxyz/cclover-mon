---
summary: "Records the decision to share dashboard presentation semantics across frontends while keeping renderer-specific layout local."
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

## Decision

Introduce a renderer-neutral presentation layer between `core::MonitorState` and concrete frontends.

The presentation layer owns dashboard semantics that should remain consistent across native/Web Iced delivery and future terminal frontends: metric grouping, display labels, value formatting, unavailable-value semantics, process rows, and access to the history associated with a displayed metric.

Each frontend owns its own layout and interaction model. The shared Iced frontend owns pixel dimensions, columns, cards, graphs, and panel sizing across native and browser/WASM runtimes. A future terminal frontend owns terminal rows, columns, focus, scrolling, and terminal rendering. Do not introduce a common renderer/widget trait merely to make these layout systems look alike.

Within a frontend, structural layout must have one authority. The Iced `PanelLayout` is the source for both block rendering and requested panel height; the application must not maintain a second hand-written copy of the panel structure.

## Rationale

Linux desktop, Windows desktop, and a future htop-style terminal interface share metric meaning but not rendering mechanics. Sharing at the presentation boundary preserves consistent semantics without coupling terminal behavior to Iced or forcing desktop layout through a lowest-common-denominator renderer abstraction.

Keeping the presentation model renderer-neutral also preserves the existing native typed data flow and avoids serialization, IPC, or duplicate derivation of rates, history, and Top-N process data.

## Consequences

- `core` remains the authority for metric semantics, derivation, history, and aggregation.
- `presentation` may depend on core model types and shared formatting rules, but not on Iced, terminal libraries, platform APIs, or app message types.
- Linux/Windows native targets and browser/WASM delivery reuse the same Iced frontend; runtime-specific hosting and transport do not fork panel drawing.
- A future terminal frontend reuses `core` and `presentation` while defining its own layout and interaction.
- Frontend layout metadata is renderer-specific and must not leak into the presentation model.
