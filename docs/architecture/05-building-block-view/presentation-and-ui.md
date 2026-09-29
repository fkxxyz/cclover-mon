---
summary: "Defines presentation semantics, shared graphical dashboard authority, and renderer responsibilities."
viewpoint: static
concerns:
  - architecture-coherence
  - portability
  - maintainability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - ui
---

# Presentation and UI

`cclover-presentation` converts `MonitorState` into renderer-neutral dashboard semantics: grouping, display values, formatting, unavailable-value semantics, process rows, and access to the corresponding history. It depends on core model types, not platform APIs or renderer toolkits.

`cclover-ui` is the graphical dashboard authority. It consumes presentation semantics and owns card/section structure, ordering, visual tokens, graph policy, and shared geometry. Any dimension that affects both rendered geometry and native-surface sizing has one definition here.

Native graphical rendering consumes `cclover-ui::NativeScene`, the shared lowering into absolute drawing primitives. Native hosts execute those primitives and provide actual text extents for their realized fonts; `cclover-ui` remains responsible for turning those measurements into cell widths, alignment, gaps, clipping, and requested surface geometry. Web consumes the higher-level dashboard tree and may use browser-native layout and drawing rather than desktop pixel primitives.

The terminal frontend consumes `cclover-presentation` directly. Terminal rows, columns, focus, scrolling, and terminal rendering are terminal-specific and are not forced through the graphical dashboard tree.

Renderer adapters realize shared decisions; they do not independently infer metric availability, ranking, formatting, graph ranges, card visibility, or graphical card structure from raw monitor state or platform identity.
