---
summary: "Defines the single-process native deployment model and target-specific dependencies."
viewpoint: deployment
concerns:
  - architecture-coherence
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

# Native Deployment

Each target builds one application executable containing the Rust application and any required compiled C++ bridge objects.

Linux and Windows builds include only their selected platform backend. External native runtime libraries depend on the selected UI toolkit and optional metric integrations.
