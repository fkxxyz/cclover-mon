---
summary: "Defines the primary runtime building blocks, ownership boundaries, and dependency direction."
viewpoint: static
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
    - core
    - platform
    - ui
    - native-bridge
---

# System Decomposition

```text
app/UI
  ↓
core: model + history + aggregation
  ↓
platform: Linux | Windows
  ↓
OS APIs / native libraries
          ↑
   optional C++ bridge
```

- **core** owns platform-neutral metric types, history, aggregation, and sampling contracts.
- **platform** owns OS-specific collection and desktop integration.
- **app/UI** consumes shared model types and owns presentation.
- **native bridge** adapts C++-only dependencies through a small C ABI.

Dependencies point toward lower layers; platform-specific types do not cross into core or shared UI.
