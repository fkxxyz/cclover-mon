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

Graphical rendering consumes `cclover-ui::Scene`, the single shared lowering into absolute logical drawing primitives. Each primitive is also the authority for its cache class and conservative damage bounds; structural primitive equality is the conservative invalidation identity. `cclover-ui` allocates text slots without renderer font measurement, then fixes alignment, gaps, clipping, graph points, and requested surface geometry. Native hosts execute those primitives with platform drawing APIs; `cclover-web-ui` mechanically serializes the same Scene to SVG and the browser only installs that markup.

The terminal frontend consumes `cclover-presentation` directly. Terminal rows, columns, focus, scrolling, and terminal rendering are terminal-specific and are not forced through the graphical dashboard tree.

Renderer adapters realize shared decisions; they do not independently infer metric availability, ranking, formatting, graph ranges, card visibility, or graphical card structure from raw monitor state or platform identity.

Dashboard section membership is structural product state, not instantaneous sample state. Once a metric section belongs to the dashboard, an empty or unavailable current collection does not remove that section or make panel height oscillate; the section remains while its metric rows may be empty. Section visibility changes only when product capability or dashboard structure changes intentionally.

Top-N subregions use observation capability as layout state. When the corresponding collection is observable, the graphical dashboard reserves the full fixed Top-N region even when fewer rows are currently populated, so sampling changes do not move surrounding content. When the collection is unavailable, the subregion is omitted entirely; the main dashboard does not spend space on unavailable placeholders or diagnostic reasons. Detailed failure reasons belong to diagnostic interfaces such as CLI probes.
