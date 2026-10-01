---
summary: "Indexes the primary runtime building blocks, ownership boundaries, and dependency direction."
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
       ↙                              ↘
terminal frontend                  cclover-ui
                                      ↓
                           graphical renderers
                           native desktop | Web
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
```

Each product composition root selects its host and transport set. The interactive root composes desktop/TUI and optional HTTP; `cclover-server` composes only the shared sampling runtime and HTTP transport. `cclover-runtime` owns the native sampler thread, latest-state publication, subscriptions, and cooperative shutdown primitive; it depends on core/platform but not on any frontend or transport. `cclover-http` subscribes to that state source and owns HTTP/Web/API projection. `cclover-platform` owns the Linux/Windows collector implementations and platform-native collection build assets; it implements core-owned collection contracts. `core` owns the platform-neutral model and sampling contracts and does not depend on `platform`. Platform-specific types remain below the platform boundary. Graphical dashboard structure has one authority in `cclover-ui`; the terminal frontend consumes presentation semantics directly because terminal layout is materially different.

Detailed building-block Views:

- [Core](core.md) — shared model, sampling, identity, process-domain projection, history, and observation semantics.
- [Platform](platform.md) — native collector composition and source ownership.
- [Presentation and UI](presentation-and-ui.md) — presentation semantics, shared graphical dashboard authority, and renderer responsibilities.
- [Desktop Host](desktop-host.md) — `cclover-desktop`, `NativeScene`, native rendering, and desktop lifecycle boundaries.
- [Transport](transport.md) — latest-state publication, browser transport, and public API projections.

Language selection and native interoperability are governed by [ADR 001](../09-architecture-decisions/001-rust-core-native-boundaries.md) and the [Platform Boundary](../08-cross-cutting-concepts/platform-boundary.md). Top-level source dependency direction is mechanically checked by `bun archgate.ts`; broader correctness remains the responsibility of the repository validation profiles and runtime evidence where applicable.
