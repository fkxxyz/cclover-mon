---
summary: "Defines completed-state publication and the independent browser and public API transport projections."
viewpoint: static
concerns:
  - architecture-coherence
  - performance
  - maintainability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - core
    - ui
---

# Transport

The native runtime owns the only sampler. Each completed typed `MonitorState` is published once to a bounded latest-state hub consumed independently by enabled native frontends.

When HTTP is enabled, the same completed state is projected into two explicit wire schemas:

- the internal rendered-dashboard transport used by SSE `/events`;
- the independently versioned public `/api/v1/*` contract.

The browser stream carries the already-rendered dashboard produced from `cclover-presentation` and `cclover-ui`; browser JavaScript is transport/DOM glue only and does not receive raw `MonitorState` or reconstruct dashboard semantics. The public API schema must not change merely because the bundled Web renderer changes. Split `/api/v1` endpoints are views over one API-v1 projection, not separate serialization authorities. Core model types are not wire schemas.

A future process-oriented Web view must be added to the shared presentation/UI authority or explicitly projected through an appropriate transport; it must not reconstruct a process domain from Top-N card lists in browser code.

Enabling desktop, terminal, or HTTP delivery never creates another sampler, collector set, or sampling cadence. Slow clients must not create unbounded state queues.
