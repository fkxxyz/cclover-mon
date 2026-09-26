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

Runtime metric flow:

```text
OS APIs / native libraries
          ↓
platform: Linux | Windows
          ↓
core: sampling + derivation + history
          ↓
        app/UI
```

Static source dependencies use inversion at the collection boundary:

```text
app composition root ───────→ core
        │                     ↑
        └────────→ platform ──┘
                       │
                       ↓
              OS APIs / native libraries
                       ↑
                optional C++ bridge
```

- **core** owns platform-neutral metric types, history, aggregation, and sampling contracts, including `Collector`.
- **platform** owns OS-specific collection and desktop integration, implements core-owned sampling contracts, and produces core-owned platform-neutral snapshots.
- **app/UI** consumes shared model types and owns presentation.
- **native bridge** adapts C++-only dependencies through a small C ABI.

The application composition root selects a platform backend and supplies it to the core sampler. `core` must not depend on `platform`; `platform` may depend on core-owned contracts and model types. Platform-specific types do not cross into core or shared UI.
