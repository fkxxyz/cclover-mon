---
summary: "Tracks direct reuse of the internal MonitorState model as the external Web transport schema."
viewpoint: assurance
concerns:
  - maintainability
  - security
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
---

# Web Transport Schema Coupling

## Status

Active — P1.

## Problem

The HTTP/SSE boundary serializes `core::model::MonitorState` directly, so the internal runtime model is also the external Web wire schema and exposure allowlist.

## Evidence

`MonitorState` and its nested snapshot/history types derive `Serialize` and `Deserialize`. `StateHub::publish` serializes the complete state with `serde_json::to_string`, and the WASM client deserializes the same type directly.

This already exposes fields that the shared panel does not consume. `SystemSnapshot::process_disk_io` and `SystemSnapshot::process_network_io` are sent to Web clients even though current `presentation`/`ui` rendering does not read them.

## Future change affected

Adding an internal metric, history field, diagnostic payload, identity detail, or other core state can unintentionally change the HTTP contract and LAN-visible data without an explicit transport decision. Conversely, wire-compatibility concerns can begin constraining otherwise-local core refactors.

## Maintenance impact

Core-model evolution, Web protocol evolution, and network exposure policy become one change axis. Reviewers cannot answer “what is exposed remotely?” from one explicit transport contract; they must audit the entire serializable core model. As the model grows, accidental protocol expansion becomes increasingly easy.

## Governing constraint

External transport schemas should be explicit projections with deliberate field ownership. Internal fields must not become network-visible merely because they are added to a shared runtime type.

## Resolution direction

Introduce the narrowest explicit Web transport projection that still lets native and WASM runtimes reuse one Iced panel implementation. Choose its architectural owner before implementation: the projection should represent the remotely rendered panel input rather than duplicate collector semantics or create a second presentation system.

Do not solve this by forking the UI or by maintaining two independent metric derivation pipelines.

## Exit criteria

The HTTP/SSE schema has an explicit field allowlist/projection separate from the internal `MonitorState`, and adding a core-only field does not change the external Web payload unless the transport projection is deliberately updated.
