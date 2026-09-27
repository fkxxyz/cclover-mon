---
summary: "Tracks duplicated eBPF map ABI definitions between C programs and Rust userspace."
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

# BPF ABI Schema Duplication

## Status

Active — P2.

## Problem

BPF map key/value layouts are independently defined in C and Rust. Correct operation depends on field order, width, padding, alignment, and semantics remaining synchronized across languages.

## Evidence

Structures such as disk/network keys and `counter_value` exist in the BPF C sources and corresponding `#[repr(C)]` Rust types in `ebpf_io.rs`. The two compilers validate their own definitions but do not establish semantic equivalence between them.

## Maintenance impact

A schema change can compile successfully on both sides while userspace decodes kernel map bytes incorrectly. Reviewers must manually compare layouts, and additional maps multiply this synchronization burden.

## Governing constraint

Cross-language ABI compatibility must be mechanically verifiable. A duplicated declaration is acceptable only when layout and version compatibility are automatically guarded strongly enough that silent drift is not plausible.

## Resolution direction

Prefer a single schema source where practical; otherwise add build/test-time size, alignment, offset, or BTF-derived verification around duplicated declarations. Keep semantic versioning explicit if incompatible schemas can coexist.

## Exit criteria

A C/Rust map-layout mismatch fails deterministically during build or test, and adding/changing a map does not rely on manual layout comparison alone.
