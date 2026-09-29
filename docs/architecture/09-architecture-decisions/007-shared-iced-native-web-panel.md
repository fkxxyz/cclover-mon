---
summary: "Records the superseded shared-Iced native/Web rendering decision and the still-current transport rationale it introduced."
viewpoint: decision
concerns:
  - architecture-coherence
  - maintainability
  - portability
  - security
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
---

# ADR 007: Shared Iced Native and Web Panel

## Status

Superseded for rendering ownership by [ADR 008](008-shared-ui-tree-platform-renderers.md). Its transport separation remains part of the current architecture and is defined by the active [Transport](../05-building-block-view/transport.md) View.

## Historical Decision

The project used one Iced panel implementation for native desktop and browser/WASM rendering. The native process remained the single sampler. When HTTP monitoring was enabled, completed `MonitorState` values were projected into an internal browser schema published through SSE `/events`; the public `/api/v1/*` surface used a separate versioned projection from the same completed state.

HTTP was opt-in and loopback-bound by default. The Web surface was read-only and deliberately separated browser transport from the public API contract.

## Rationale at the Time

A shared Iced renderer avoided duplicating visual structure across native and Web targets. SSE matched one-way, low-frequency state delivery and complete bounded projections kept reconnect behavior simple. Serialization remained confined to the network boundary; native rendering continued to use typed in-process state.

Keeping browser transport and public API schemas independent prevented bundled-client evolution from silently changing the externally versioned API. Both transports reused one native sampler and bounded state publication.

## Supersession

ADR 008 replaced the shared-Iced renderer with one renderer-neutral graphical dashboard tree, platform-native desktop renderers, and browser-native DOM/CSS/SVG realization. The historical patched-Iced dependency and WebGL rendering workaround are no longer part of the dependency graph.

Current rules for sampler ownership, browser/public projections, bounded client delivery, and external transport exposure are defined by [Transport](../05-building-block-view/transport.md). Current graphical ownership is defined by [Presentation and UI](../05-building-block-view/presentation-and-ui.md) and ADR 008.
