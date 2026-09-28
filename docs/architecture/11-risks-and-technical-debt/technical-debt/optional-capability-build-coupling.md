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

Resolved.

## Problem

Capabilities that are optional or degradable at runtime still impose mandatory toolchain, dependency, and artifact costs on normal native builds.

## Evidence

The default/full build enables both `http` and `ebpf-io`, preserving the existing release artifact: HTTP embeds the browser bundle, and Linux eBPF attribution compiles and links its BPF/libbpf path.

A supported minimal native build uses `--no-default-features`. It omits the optional `wasm-bindgen-cli-support` build dependency, skips the nested `wasm32-unknown-unknown` build, does not compile the eBPF objects, and does not link libbpf. `--http` is rejected explicitly, while disk/network process attribution reports `Disabled`; unrelated native metrics and frontends continue to build and run.

## Maintenance impact

Developers changing unrelated native UI/core code, minimal packages, cross-compilation environments, and CI jobs inherit toolchains and build work for capabilities they may not exercise. Build failures in an optional capability can block unrelated native development. As more optional integrations are added, this pattern can turn the default build into the union of every toolchain and packaging requirement in the project.

## Governing constraint

Runtime-optional capabilities should not automatically become mandatory build dependencies unless that cost is an explicit supported-build baseline with demonstrated packaging value.

## Resolution

The supported build matrix is explicit. Default/full native builds retain HTTP/Web, eBPF attribution, and the single-process/single-executable deployment model. Minimal native builds omit both optional capabilities with Cargo features and therefore avoid their toolchains and generated artifacts without changing unrelated functionality.

## Exit criteria

For each runtime-optional capability, either:

- its build dependency is explicitly adopted and validated as part of every supported native build baseline; or
- supported native builds can omit it without requiring its toolchain or generated assets, while full release artifacts retain the intended capability.
