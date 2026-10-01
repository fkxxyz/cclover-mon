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

`cclover-runtime` owns the process's only sampler. Each completed typed `MonitorState` is published once to a bounded latest-state source consumed independently by enabled native frontends and transports. Product composition roots create one runtime instance; transports do not create samplers.

When `cclover-http` is present, it subscribes to that completed state and projects it into two explicit wire schemas:

- the internal rendered-dashboard transport used by SSE `/events`;
- the independently versioned public `/api/v1/*` contract.

The browser stream carries the already-rendered dashboard produced from `cclover-presentation` and `cclover-ui`; browser JavaScript is transport/DOM glue only and does not receive raw `MonitorState` or reconstruct dashboard semantics. The public API schema must not change merely because the bundled Web renderer changes. Split `/api/v1` endpoints are views over one API-v1 projection, not separate serialization authorities. Core model types are not wire schemas.

A future process-oriented Web view must be added to the shared presentation/UI authority or explicitly projected through an appropriate transport; it must not reconstruct a process domain from Top-N card lists in browser code.

Enabling desktop, terminal, or HTTP delivery never creates another sampler, collector set, or sampling cadence. Slow clients must not create unbounded state queues. One application-level `cclover-runtime::Shutdown` authority drives sampler, HTTP listener, state projection, SSE/client teardown, and process-host shutdown; long-lived HTTP workers consume event notifications rather than periodic shutdown polling. HTTP connection concurrency is bounded and client threads are joined during teardown. The HTTP projection has no synthetic initial state: `/healthz` can report listener liveness before sampling, while `/readyz`, `/events`, and `/api/v1/*` wait for the first real published `MonitorState`.
