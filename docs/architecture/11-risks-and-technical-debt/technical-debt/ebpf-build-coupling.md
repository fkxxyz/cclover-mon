---
summary: "Tracks mandatory Linux build coupling for an eBPF feature that is optional at runtime."
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

# eBPF Build Coupling

## Status

Active — P2.

## Problem

Linux eBPF I/O attribution is degradable/optional at runtime, but the normal Linux build unconditionally compiles BPF objects with clang and links libbpf.

## Evidence

`build.rs` always compiles disk/network BPF sources for Linux and emits `cargo:rustc-link-lib=bpf`; runtime disablement only affects loading/use after the binary has already been built.

## Maintenance impact

Developers changing unrelated UI/core code, minimal packages, cross-compilation environments, and CI jobs still require the full BPF toolchain and libbpf development environment. An optional runtime capability therefore expands the build dependency surface of the whole Linux application.

## Governing constraint

Optional native capabilities should not impose unrelated build/toolchain requirements unless that dependency is an explicit supported-build baseline with demonstrated packaging value.

## Resolution direction

Decide whether eBPF tooling is intentionally mandatory for every Linux build. If not, introduce a build/package boundary that can omit attribution while preserving clear feature semantics and supported artifact combinations.

## Exit criteria

Either the mandatory BPF build dependency is explicitly adopted and validated as a project baseline, or supported Linux builds can omit eBPF attribution without requiring clang/libbpf while the rest of the application remains functional.
