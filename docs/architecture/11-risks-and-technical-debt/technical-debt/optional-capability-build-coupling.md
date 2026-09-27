---
summary: "Tracks runtime-optional capabilities that remain mandatory build dependencies for normal native artifacts."
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

# Optional Capability Build Coupling

## Status

Active — P2.

## Problem

Capabilities that are optional or degradable at runtime still impose mandatory toolchain, dependency, and artifact costs on normal native builds.

## Evidence

Two current capabilities have this shape:

- Linux eBPF I/O attribution is degradable at runtime, but normal Linux builds unconditionally compile BPF objects with clang and link libbpf.
- HTTP/Web monitoring is disabled by default at runtime, but every native build unconditionally compiles the shared Iced frontend for `wasm32-unknown-unknown`, runs `wasm-bindgen`, and embeds the generated JavaScript/WASM assets into the native executable.

The Web path therefore requires the WASM target and bindgen build dependencies even for developers or packaging jobs that never enable `--http`, and it increases every native artifact by the embedded Web bundle.

## Maintenance impact

Developers changing unrelated native UI/core code, minimal packages, cross-compilation environments, and CI jobs inherit toolchains and build work for capabilities they may not exercise. Build failures in an optional capability can block unrelated native development. As more optional integrations are added, this pattern can turn the default build into the union of every toolchain and packaging requirement in the project.

## Governing constraint

Runtime-optional capabilities should not automatically become mandatory build dependencies unless that cost is an explicit supported-build baseline with demonstrated packaging value.

## Resolution direction

Define the supported build matrix before introducing a large abstraction. Decide which capabilities are mandatory in release artifacts versus optional in developer/minimal builds, then introduce the narrowest build/package boundary that preserves the single-process/single-executable deployment model for full builds.

Possible mechanisms include Cargo features, dedicated packaging profiles, or prebuilt/generated asset stages, but the mechanism should follow the supported artifact policy rather than precede it.

## Exit criteria

For each runtime-optional capability, either:

- its build dependency is explicitly adopted and validated as part of every supported native build baseline; or
- supported native builds can omit it without requiring its toolchain or generated assets, while full release artifacts retain the intended capability.
