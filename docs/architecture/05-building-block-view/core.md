---
summary: "Defines core ownership of typed state, sampling semantics, stable identity, process-domain projection, and bounded history."
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
---

# Core

`core` owns platform-neutral metric types, the `Collector` contract, delta/rate derivation, aggregation, bounded history, sampling policy, and stable semantic identity. Platform backends implement core-owned contracts and return core-owned snapshots; `core` never imports platform implementations.

Every raw metric uses the typed `Collection<T>` outcome. `Available(T)` is a complete observation, `Degraded(T)` a usable partial observation, and `Unavailable(reason)` no observation. A successful empty collection is data, not failure. Downstream code must preserve distinctions that affect behavior rather than collapse unavailable observations into zero, empty data, or an unqualified `None`.

Any state that survives one observation is associated by an explicit stable identity. `ProcessInstanceId`, `GpuId`, `NetworkId`, and `DiskId` are core-owned identities; PID, display labels, enumeration order, device names, and other native locators are not substitutes unless their stability is part of the declared identity contract.

Physical-disk observations keep display metadata separate from `DiskId`: `system_label` is a short OS-level physical-device label and `associated_labels` are logical-storage labels related by platform topology. Neither field participates in delta derivation or history keys, so label changes cannot reset physical-disk continuity.

`ProcessDomainSnapshot`, keyed by `ProcessInstanceId`, is the authoritative process-centric projection. Each cycle joins available process metadata, CPU and memory, per-disk I/O, and per-interface network I/O before dashboard-oriented ranking. The domain uses the union of identities reported by process and attribution sources so an attribution-only process remains representable with missing metadata. Capability status remains explicit. The complete keyed map is shared behind `Arc`; CPU, memory, disk, and network card lists are derived projections rather than parallel authorities.

Sampling orchestration coordinates snapshot-wide flow while metric-specific derivation remains separable and independently testable. History uses bounded storage, and cross-sample derivation never spans an unavailable observation as if it were a valid baseline.
