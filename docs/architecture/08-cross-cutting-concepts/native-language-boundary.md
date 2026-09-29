---
summary: "Defines safe Rust ownership and the narrow C/C++ interoperability boundary."
viewpoint: static
concerns:
  - architecture-coherence
  - performance
  - portability
  - maintainability
  - security
activities:
  - orient
  - change
  - assess
facets:
  area:
    - native-bridge
---

# Native Language Boundary

Rust source is safe by default: crates deny `unsafe_code`. Raw FFI, pointer manipulation, dynamic symbol loading, and native lifetime mechanics may opt out only in narrowly scoped native adapter modules. Unsafe blocks and unsafe trait implementations document their local `SAFETY` invariant; metric policy, availability semantics, shaping, and cross-sample logic stay on the safe side of that boundary.

Language choice follows responsibility. Rust owns application semantics and native resources whose lifetime, synchronization, parsing, identity, or failure semantics materially benefit from safe ownership. C is appropriate for thin native hosting or API/protocol translation when it removes meaningful framework, runtime, dependency, binary-size, or interoperability cost without moving application policy into C. C++ is limited to compatibility shims for C++-only dependencies and remains behind a C ABI.

Every Rust/C boundary exchanges explicit C-compatible contracts such as POD records, fixed-layout buffers, opaque handles, status codes, and callbacks. Rust-owned collections, trait objects, allocator ownership, and language-specific object graphs do not cross directly.

Detailed selection criteria and rationale are authoritative in [ADR 001](../09-architecture-decisions/001-rust-core-native-boundaries.md); this View records the active boundary contract rather than duplicating that decision procedure.
