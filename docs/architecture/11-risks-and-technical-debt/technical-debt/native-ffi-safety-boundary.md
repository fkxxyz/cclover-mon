---
summary: "Tracks raw native FFI and unsafe lifecycle code mixed with metric-specific collection semantics."
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

# Native FFI Safety Boundary

## Status

Active — P2.

## Problem

Raw FFI operations and unsafe lifecycle mechanics are mixed into modules that also own metric discovery and shaping semantics. The unsafe review surface therefore grows with feature logic instead of remaining a narrow adapter boundary.

## Evidence

`ebpf_io.rs` combines raw libbpf pointers/functions, `unsafe impl Send`, RAII, map access, and disk/network attribution semantics. NVML collection combines `dlopen`/`dlsym`, function-pointer loading/transmutation, session lifecycle, device discovery, and temperature shaping.

## Maintenance impact

Adding native calls or changing metric behavior requires reviewers to reason about safety invariants and domain behavior simultaneously. Similar future integrations can copy this pattern and expand unsafe surface area.

## Governing constraint

Unsafe/native ABI code should expose the narrowest practical safe interface to metric collectors. Safety invariants and resource ownership must be reviewable independently from product-level metric semantics.

## Resolution direction

Factor small native runtime/session adapters where this reduces unsafe surface without creating generic wrappers detached from the actual library contract. Keep metric-specific policy on the safe side of the boundary.

## Exit criteria

Metric collection logic can evolve primarily through safe APIs, and raw pointer/function-loading/resource-lifetime invariants are concentrated in narrowly scoped native adapters.
