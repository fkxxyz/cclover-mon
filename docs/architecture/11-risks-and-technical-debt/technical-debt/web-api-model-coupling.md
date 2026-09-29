---
summary: "Tracks public Web API schemas that evolve too directly with internal monitor-state and metric-model changes."
viewpoint: assurance
concerns:
  - maintainability
  - portability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
---

# Web API Model Coupling

## Status

Resolved.

## Root cause

The versioned HTTP API is mapped too directly from internal monitor-state and metric-history structures. Internal model evolution therefore tends to become wire-schema evolution even when the public API does not need to change.

## Evidence

The GPU model change from a memory-only entity to a unified GPU entity propagated through Web transport snapshot/history DTOs and split API slices. Compatibility paths such as `/api/v1/gpu-memory` can remain routable while returning the newer GPU-shaped payload, so URL compatibility alone does not establish schema compatibility.

## Governing constraint

A versioned public API contract must have an explicit schema authority independent from internal Rust model layout. Internal types may evolve without changing the `/api/v1` wire contract unless an API change is intentional and compatibility is handled explicitly.

## Scope discovery

Review every `/api/v1` state/history endpoint and its serialized DTOs. For each field, determine whether its shape is intentionally part of the public contract or merely mirrors an internal `MonitorState`, snapshot, history, or presentation structure. Include compatibility aliases and full-state serialization, not only split endpoints.

## Maintenance consequence

Reasonable internal refactors or metric extensions can silently become breaking API changes. As more external consumers appear, model cleanup, capability expansion, and metric renaming become constrained by accidental wire compatibility, while old route aliases can give a false impression of backward compatibility.

## Resolution

`src/web_api.rs` now owns the `/api/v1` schema independently from `src/web_transport.rs`, which remains the internal browser/SSE transport. Both are explicit projections from the same completed `MonitorState`; neither serializes core model types directly. `/api/v1/state` and every split domain/history endpoint are views over one `ApiV1State` authority, so adding or restructuring internal fields cannot silently alter v1 unless the API projection is deliberately changed.

The current unified-GPU shape is the v1 baseline for `/api/v1/state` and `/api/v1/gpus`. The legacy `/api/v1/gpu-memory` and `/api/v1/history/gpu-memory` routes are no longer mere URL aliases: they project the historical GPU-memory payload shape, including `gpu_memory`, `used_bytes`/`total_bytes`, and `gpu_memory_used`. GPU devices lacking a complete memory pair are omitted from that legacy list because the historical row contract cannot represent partial memory telemetry.

Representative contract tests compare complete JSON values for unified GPU payloads, collection outcome encoding, legacy GPU-memory compatibility, and HTTP route behavior. The API DTOs are serialization-only; reverse deserialization remains confined to the bundled browser transport where it is actually required.

## Exit criteria

All `/api/v1` payloads have an explicit API-owned contract, and representative compatibility tests prove that internal model changes do not alter existing wire shapes unless the API version or documented contract changes intentionally. Legacy route aliases, if retained, are verified for payload compatibility rather than only successful routing.
