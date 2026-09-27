---
summary: "Defines the observable quality requirements that drive cclover-mon architecture."
viewpoint: assurance
concerns:
  - performance
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

# Quality Requirements

| Quality | Scenario |
|---|---|
| Performance | Adding a metric does not require subprocess polling, per-widget collection, or serialization between collection and UI. |
| Efficiency | Sampling reuses bounded storage and avoids work proportional to UI object count. |
| Portability | Linux and Windows produce the same shared semantic model without leaking native API types into shared code. |
| Extensibility | A new native metric source is added behind the platform or bridge boundary without changing unrelated collectors or UI contracts. |
| Frontend reuse | Desktop and future terminal frontends consume the same renderer-neutral dashboard presentation without duplicating rate derivation, Top-N aggregation, unavailable-value semantics, or common value formatting. |
| Layout authority | A frontend structural change has one layout authority; the Iced block structure used to render the desktop panel is also the structure used to derive its requested panel height. |
| Diagnosability | A developer can distinguish native collection, derivation, runtime timing, and presentation failures using `probe <collector> [--raw]`, `dump`, development logs, and screenshots respectively. |
| Collector isolation | A collector can be exercised independently through the same production collector implementation, with elapsed time and failure/skip reasons visible without starting the GUI. |
| Sampling health | A sampling cycle that exceeds its configured interval produces an overrun diagnostic containing actual duration and target interval. |
| Diagnostic overhead | Development observability does not require a background metrics service, persistent logging pipeline, subprocess polling, or a second metric transport. |
| UI iteration | Pure visual changes can use reloadable resources when supported by the selected UI toolkit. |
