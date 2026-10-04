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

Structured metric values placed in fixed graphical slots use presentation-owned bounded compact text: the formatter declares a conservative maximum monospace-column count and guarantees every emitted compact value stays within it. Presentation may also expose a fuller representation for frontends such as the TUI that are not governed by graphical fixed-slot geometry. `cclover-ui` owns the matching `TextSlot` capacity and derives its pixel width from that capacity with the shared conservative 5/8-em monospace advance budget. Each slot also records the maximum logical font size used to derive that width, and graphical value construction rejects a larger size. Fixed width and clipping are independent semantics: bounded metric values must fit their slot and are never silently clipped, while unbounded identity text such as device or process names may opt into clipping explicitly. Renderers execute these shared bounds rather than remeasure text and create renderer-specific geometry.

The terminal frontend consumes `cclover-presentation` directly. Terminal rows, columns, focus, scrolling, and terminal rendering are terminal-specific and are not forced through the graphical dashboard tree.

Renderer adapters realize shared decisions; they do not independently infer metric availability, ranking, formatting, graph ranges, card visibility, or graphical card structure from raw monitor state or platform identity.

Dashboard section membership is structural product state, not instantaneous sample state. Once a metric section belongs to the dashboard, an empty or unavailable current collection does not remove that section or make panel height oscillate; the section remains while its metric rows may be empty. Section visibility changes only when product capability or dashboard structure changes intentionally.

Top-N subregions use observation capability as layout state. When the corresponding collection is observable, the graphical dashboard reserves the full fixed Top-N region even when fewer rows are currently populated, so sampling changes do not move surrounding content. When the collection is unavailable, the subregion is omitted entirely; the main dashboard does not spend space on unavailable placeholders or diagnostic reasons. Detailed failure reasons belong to diagnostic interfaces such as CLI probes.
