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

- the internal browser transport used by SSE `/events`;
- the independently versioned public `/api/v1/*` contract.

The browser transport may evolve with the bundled Web client. The public API schema must not change merely because core or browser-transport types change. Split `/api/v1` endpoints are views over one API-v1 projection, not separate serialization authorities. Core model types are not wire schemas.

The current browser dashboard transport carries compact card-oriented process projections. A future process-oriented Web view must explicitly project `ProcessDomainSnapshot` rather than reconstruct a process domain from Top-N card lists.

Enabling desktop, terminal, or HTTP delivery never creates another sampler, collector set, or sampling cadence. Slow clients must not create unbounded state queues.
