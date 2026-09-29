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

Active — P2.

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

## Repair direction

Define API-owned DTOs and compatibility expectations explicitly, then map internal state into those DTOs. Keep the mapping narrow and deliberate; do not introduce a second domain model beyond what is needed to stabilize the wire contract.

## Exit criteria

All `/api/v1` payloads have an explicit API-owned contract, and representative compatibility tests prove that internal model changes do not alter existing wire shapes unless the API version or documented contract changes intentionally. Legacy route aliases, if retained, are verified for payload compatibility rather than only successful routing.
