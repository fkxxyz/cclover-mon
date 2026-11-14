---
summary: "Puts process instance identity on every process-facing API row and moves ranked process views out of the sampled snapshot, so card layout policy no longer shapes the public contract."
viewpoint: decision
concerns:
  - architecture-coherence
  - maintainability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
    - core
---

# ADR 017: Process Identity on the Public API and Ranking Ownership

## Context

`cclover-mon` publishes a public `/api/v1` contract next to its own dashboard. The sampled
`SystemSnapshot` used to carry four materialized "card projections" — top CPU rows, top memory
rows, and the highest process-I/O rows per disk and per network interface — produced inside the
sampling cycle with row counts fixed at 8 and 3.

That made the dashboard's layout policy the shape of the shared model, and it made the API a
serialization of dashboard rows rather than of observations. Two consequences were visible:

- The CPU and memory projections reduced each row to a display name plus a value, so the process
  identity was discarded before any consumer could see it. Two instances of the same executable
  became indistinguishable, and a client could not correlate an API row with a process at all.
- The I/O projections did carry identity, but as a raw `birth_marker` JSON number. On Windows that
  marker is a process creation timestamp in 100-ns units since 1601, far outside the exactly
  representable JSON number range, so clients reading it as a double silently lost low bits.

Identity is not a presentation detail: it is the same `ProcessInstanceId` that core uses to join
observations, refuse PID reuse, and key history. An API that drops it cannot express what the
product already knows.

## Decision

**Every process-facing API row carries process instance identity.** CPU, memory, per-disk I/O, and
per-interface network I/O rows all begin with one `process` object:

```json
{"process": {"pid": 1234, "instance": "133012345678901234"}, "name": "chrome", "percent": 37.5}
```

- `instance` is the core `ProcessInstanceId` generation marker, serialized as a decimal **string**.
  The contract is stability and uniqueness per process instance only: it is stable for one
  instance, differs when a PID is reused, and must not be parsed as a time. Its platform unit stays
  private, and a string keeps the wire value exact on every platform and leaves room for a future
  encoding.
- `name` is always present and nullable. A row is never dropped because metadata was not observed;
  attribution-only processes remain visible with `name: null`.
- Wire key names are unchanged. `snapshot.top_cpu`, `snapshot.top_memory`,
  `snapshot.process_disk_io`, and `snapshot.process_network_io` still exist in `/api/v1/state`, and
  the split endpoints keep their existing keys. Only the row shape changed.

**Ranked process views are queries over the process domain, not snapshot fields.**
`SystemSnapshot` no longer stores any ranked process projection. `ProcessDomainSnapshot` exposes
`cpu_ranking(limit)`, `memory_ranking(limit)`, `disk_io_ranking(per_device_limit)`, and
`network_io_ranking(per_device_limit)`. They return the same status envelope as the observation
they rank, keep ordering deterministic (descending value, then ascending `ProcessInstanceId`), and
carry `ProcessRef` so identity always travels with a row.

Row counts therefore belong to whoever consumes the ranking:

- `cclover-presentation` owns the dashboard card row counts (`PROCESS_CARD_ROWS`,
  `IO_CARD_ROWS`) because they are card layout policy, and evaluates each ranking once per
  published state.
- `cclover-http` owns the wire default row counts, because the array length of a default v1 payload
  is part of the published contract. The two policies are intentionally separate even where they
  currently agree.

Core still owns ranking semantics: ordering, the per-device grouping for I/O, status composition,
and identity. No consumer re-derives rates, attribution joins, or platform lookups.

The `ProcessInstanceId` field is renamed from `birth_marker` to `instance` for the same reason the
wire value is a string: it is a generation marker, not a birth time.

This is a deliberate in-place v1 change rather than a v2. The contract has no consumer outside
this repository, and that is a confirmed fact rather than an assumption: when the same review
removed the legacy GPU-memory endpoints, the maintainer confirmed that nothing external reads
`/api/v1` at all. The affected shape (`birth_marker` as a number) is the defect being fixed, so
keeping a compatibility alias would preserve the defect for no audience. There is no release-note
channel in this repository and none is needed while the contract has no external consumer; if one
appears, the requirement returns with it.

## Consequences

Positive:

- An API client can identify, correlate, and follow a process instance. Duplicate executable names
  are distinguishable, and PID reuse is observable instead of invisible.
- The shared model no longer stores dashboard layout policy, so API evolution no longer needs to
  argue with card geometry, and card layout no longer needs to argue with wire compatibility.
- One row shape and one identity carrier replaced two parallel projection families and a duplicated
  `(identity, name)` pair in four row types.
- The sampling cycle no longer ranks processes that nobody may consume; ranking happens per
  consumer and per published state.

Negative:

- `Dashboard` now owns the ranked rows it borrows, so it is no longer `Copy` and its accessors take
  `&self`. `DashboardUi::new` takes `&Dashboard`, and callers must keep the dashboard alive.
- Ranking is recomputed per consumer instead of once per sample. The domain holds hundreds to a few
  thousand rows and each consumer ranks once per published state, so this is negligible, but it is
  a real change in where the work happens.
- The dashboard card and the default API payload now hold separate row-count constants. They are
  two different policies that currently agree; a future change to one is a product decision rather
  than an automatic propagation.

## Decisions taken with this change

Three product boundaries were settled while this decision was made, and they are recorded here so a
later session does not re-open them by accident:

- **The graphical dashboard keeps its current process row appearance.** The panel shows process
  names without a pid column, so two instances of the same executable remain visually
  indistinguishable there. The compact card gives a process name roughly fifteen monospace columns,
  and an appended `·<pid>` would take about seven of them. Identity is available in the API and in
  the terminal frontend instead. Presentation already carries the identity, so showing it later is a
  visual decision that needs no core or wire change.
- **The default row counts stay fixed; there is no `limit` query parameter.** The API publishes
  eight CPU and memory rows and three rows per device for I/O, and that array length is the
  contract. A caller-selectable limit was considered and deferred because it needs a bound, a
  pagination story, and a per-request projection path; the ranking functions already accept a
  limit, so adding one later is additive.
- **Duplicate display names are an accepted gap, not a defect.** Nothing in the dashboard
  disambiguates two same-named processes; consumers that need identity use the API or the terminal
  frontend.

## Reassessment conditions

Revisit this decision if:

- an external consumer appears for `/api/v1`, which immediately restores the need for a release-note
  channel and a compatibility discipline for breaking revisions;
- a consumer needs more rows than the default payload carries, which requires a documented query or
  pagination contract rather than a shared constant;
- a consumer needs an arbitrary process query (by PID, by sort key, or the full domain), which is a
  new transport view rather than another ranking;
- another frontend needs ranked process rows, at which point the per-consumer evaluation cost and
  the duplication of row-count policy must be re-examined;
- the wire needs a human-readable process start time, which is a new platform-derived field and not
  a reinterpretation of `instance`.

Two consequences above are retained debt rather than accepted design, and this decision owns their
re-evaluation triggers:

- **Row-count constants are duplicated across three layers** — the presentation card budgets, the
  HTTP wire defaults, and the core test helpers. No mechanism makes divergence visible, so a change
  to one can silently disagree with the published contract. Repair when the row count or a payload
  length next changes: one cross-layer assertion that pins the published length, replacing the bare
  literals.
- **The contract document is a second, unchecked representation of the wire** — `docs/api/v1.md`
  restates field shapes and examples that the projection types own. It names the test that pins the
  examples, which makes the authority discoverable but not enforced. Mechanize the comparison, or
  move the examples out of the document, when the wire shape next changes.
- **Ranking semantics are proven at two levels** — direct unit tests over the domain and
  pipeline-level tests through derivation both assert ordering and truncation. Decide retain,
  migrate, or retire when ranking semantics next change; the pipeline proof additionally covers the
  derivation chain, which the unit proof does not.
