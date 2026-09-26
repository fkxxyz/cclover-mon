---
summary: "Records the decision to use Rust as the primary implementation language with narrow C++ bridges for C++-only dependencies."
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

# ADR 001: Rust Core with C++ Bridges

## Decision

Use Rust for the application and platform backends. Introduce C++ only when an integration requires a C++ API, exposing that integration to Rust through a narrow C ABI. Cargo owns the final build and link.

## Rationale

Rust provides native performance, compile-time safety, and zero-cost static abstraction for the main codebase. A local C++ bridge preserves access to C++-only SDKs without making C++ the system-wide ownership model.

## Consequences

The project maintains one primary language and accepts small native bridge modules where ecosystem compatibility requires them.
