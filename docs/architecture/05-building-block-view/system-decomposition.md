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
presentation: renderer-neutral dashboard semantics
          ↓
frontends: Iced desktop | future terminal
```

Static source dependencies use inversion at the collection boundary:

```text
app composition root ───────→ frontend
        │                        ↓
        ├────────→ presentation ─┘
        │              ↓
        ├────────────→ core
        │              ↑
        └────────→ platform
                       │
                       ↓
              OS APIs / native libraries
                       ↑
                optional C++ bridge
```

- **core** owns platform-neutral metric types, history, aggregation, and sampling contracts, including `Collector`.
- **platform** owns OS-specific collection and desktop integration, implements core-owned sampling contracts, and produces core-owned platform-neutral snapshots. A platform backend is the batch composition point; metric-specific native IO, parsing, and mutable state belong to the corresponding metric responsibility rather than the backend itself.
- **presentation** converts `MonitorState` into renderer-neutral dashboard semantics: panel meaning, display values, shared formatting, and access to the corresponding history. It depends on core model types but not on Iced, terminal libraries, platform APIs, or application messages.
- **frontend** owns renderer-specific widgets, interaction, and layout. The Iced frontend is shared by Linux and Windows desktop builds. A future terminal frontend may consume the same presentation model while owning terminal-specific layout and interaction.
- **app** composes the selected platform collector, shared presentation, and frontend lifecycle. Frontends do not depend back on application message types when they only render data.
- **native bridge** adapts C++-only dependencies through a small C ABI.

The application composition root selects a platform backend and supplies it to the core sampler. `core` must not depend on `platform`; `platform` may depend on core-owned contracts and model types. Platform-specific types do not cross into core, presentation, or shared frontends.

Renderer-specific layout has one authority inside each frontend. For the Iced desktop panel, one `PanelLayout` structure determines both which blocks render and the requested layer-surface height. Pixel dimensions do not belong to the renderer-neutral presentation model.

Within a platform backend, dependencies point from the backend to independent metric collectors:

```text
platform backend
  ├── CPU collector
  ├── memory collector
  ├── process collector
  ├── network collector
  ├── disk collector
  └── temperature collector
```

The backend may know every collector so it can assemble a batch snapshot. A metric collector does not depend on the backend or on sibling collectors.
