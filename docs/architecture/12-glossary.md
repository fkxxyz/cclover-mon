---
summary: "Defines canonical terminology used by the cclover-mon Architecture Description."
viewpoint: overview
concerns:
  - architecture-coherence
activities:
  - orient
facets:
  area:
    - whole-system
---

# Glossary

| Term | Meaning |
|---|---|
| Snapshot | One typed, platform-neutral observation of current monitored state. |
| Platform backend | Linux- or Windows-specific implementation that converts native system data into shared model types. |
| Native bridge | Thin interoperability layer used when a dependency exposes a C++-only API. |
| Sampling cycle | One coordinated collection pass followed by aggregation and history update. |
| Shared UI | Presentation code maintained once across supported platforms. |
