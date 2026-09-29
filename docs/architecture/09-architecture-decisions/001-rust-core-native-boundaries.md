---
summary: "Defines language selection: Rust for safety-critical state and lifetime semantics, C for thin native adaptation, and C++ only for C++-only dependencies."
viewpoint: decision
concerns:
  - performance
  - portability
  - maintainability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
    - native-bridge
---

# ADR 001: Rust Safety Core with Native C Boundaries

## Decision

Use Rust where the implementation materially benefits from memory safety, ownership and lifetime checking, typed state, concurrency safety, or robust parsing. Use C for thin native adaptation whose main job is to call an OS, display, terminal, or desktop protocol API and translate between that API and a narrow project-owned contract. Use C++ only when an external dependency exposes a C++-only API; keep that code behind a narrow C ABI. Cargo remains the final build and link authority.

Language is selected by responsibility, not by layer name. A platform module is not automatically C, and native API usage is not automatically Rust.

### Choose Rust when

- state or resources have non-trivial ownership, lifetime, shutdown, or concurrency semantics;
- code owns cross-sample or cross-frontend state, aggregation, history, identity, availability, or other application semantics;
- the implementation parses variable or potentially malformed text/binary input and gains real safety from bounds-checked data handling;
- a small Rust adapter can wrap unsafe native handles or dynamic symbols and expose a safe API without bringing in a disproportionate dependency stack;
- moving the code to C would mostly reproduce existing Rust logic without removing a meaningful dependency, runtime, or binary-size cost.

This includes the shared core and presentation semantics, Linux `/proc` and `/sys` parsing, long-lived eBPF/libbpf handle ownership, and the dynamically loaded NVML session/device lifetime.

### Choose C when

- the work is predominantly a thin call/translation layer over a stable native C ABI or operating-system interface;
- behavior is naturally event-loop or callback glue owned by the native platform boundary rather than application semantics;
- a Rust implementation would require a comparatively large framework, async runtime, protocol stack, terminal toolkit, or similar abstraction for a small amount of product behavior;
- the Rust/C boundary can remain narrow and exchange POD records, buffers, opaque handles, status codes, and callbacks;
- the resulting implementation measurably reduces dependency surface, binary size, build complexity, or platform impedance without moving application policy into C.

Native desktop hosting is C. Terminal hosting/rendering and Linux StatusNotifierItem integration should follow the same rule when implemented without the current high-level Rust stacks.

### Choose C++ only when

A required external SDK or library is C++-only. The C++ module must be a compatibility shim, not a second application architecture:

```text
Rust → C ABI → thin C++ bridge → C++ library / SDK
```

C++ types, exceptions, RTTI-dependent interfaces, templates, and STL containers do not cross the project ABI.

## Decision procedure for new work

Before adding a dependency or choosing a language, identify the smallest responsibility that owns the behavior and ask in this order:

1. Does the responsibility contain application state, lifetime, parsing, synchronization, identity, derivation, or failure semantics? Keep that part in Rust.
2. Is the remaining work mostly native API/protocol invocation and translation? Prefer C.
3. Would C actually remove meaningful Rust dependency/runtime/code-size cost, or merely rewrite safe Rust in C? If there is no material simplification, keep Rust.
4. Is C++ required by an external C++-only dependency? If not, do not introduce C++.
5. If code crosses a language boundary, keep application policy on the Rust side and make the ABI narrow, explicit, and mechanically verifiable where layout matters.

Do not generalize one successful C migration into a rule that all platform code belongs in C. The governing rule is to keep safety- and state-heavy semantics in Rust while moving low-value wrapper abstraction to the native language boundary.

## Rationale

Rust provides the most value where invalid ownership, lifetime, mutation, parsing, or synchronization states are otherwise expensive to prevent. C provides the most value where the operating system already exposes a C ABI and the project only needs a small adapter. Keeping those responsibilities separate avoids paying for broad Rust ecosystem abstractions merely to wrap a small native surface, while also avoiding unsafe rewrites of logic that already benefits from Rust.

The decision is therefore optimized for total implementation cost: executable size, dependency graph, build complexity, native interoperability, and correctness risk—not source-language purity.

## Consequences

The project intentionally uses both Rust and C. New native adapters may be added in C when they satisfy the criteria above. Existing Rust code should migrate to C only when the same criteria demonstrate a material benefit. C++ remains exceptional and dependency-driven.
