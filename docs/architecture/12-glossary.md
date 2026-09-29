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
| Native boundary | Cross-language/platform seam: C for thin native OS/API/protocol adapters when justified by dependency or interoperability cost; C++ only as a compatibility shim for C++-only dependencies. |
| Sampling cycle | One coordinated collection pass followed by aggregation and history update. |
| Shared UI | Presentation code maintained once across supported platforms. |
