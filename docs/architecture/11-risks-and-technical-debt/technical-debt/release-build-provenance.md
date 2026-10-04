---
summary: "Records incomplete build-environment provenance for published release artifacts."
viewpoint: assurance
concerns:
  - maintainability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
---

# Release Build Provenance

**Priority:** Low-medium

## Root cause

Release manifests identify product, version, Rust target, Git commit, archive name, and SHA-256, but do not identify the concrete toolchain and build-environment versions that produced the bytes. Release automation currently relies on mutable tool selections such as the stable Rust toolchain, hosted runner images, and installed cargo-xwin/tool packages.

## Optimization dimension

Primary: observability and diagnosis efficiency. Secondary: operations and lifecycle efficiency.

## Current cost

If a historical release exhibits a compiler-, linker-, SDK-, or packaging-tool-specific defect, the source commit and archive hash establish identity but not enough build context to reproduce or compare the artifact confidently. Maintainers must reconstruct historical CI environment details from workflow logs and external runner/tool history, increasing diagnosis time and weakening long-term reproducibility.

## Evidence

The current aggregate release manifest records the common Git commit and per-artifact SHA-256 but not `rustc`/Cargo identity, cargo-xwin version, runner image identity, native compiler/linker versions, or packaging-tool versions. CI setup intentionally follows stable/hosted tooling rather than an immutable environment snapshot.

## Reachable better state

Record a compact machine-readable build-provenance block for each release containing the small set of tool identities that can materially affect output: Rust compiler/Cargo identity, cargo-xwin for Windows artifacts, runner/OS image identity, and relevant native packaging/compiler tools. Pin tools whose uncontrolled movement creates demonstrated release risk; recording provenance is sufficient for lower-risk tools.

## Governing constraint

A published artifact must carry enough immutable build provenance that a maintainer can identify the materially relevant compiler/tool environment that produced it without relying on ephemeral CI logs.

## Scope discovery

Cover Rust toolchain setup, cargo-xwin installation, Windows CRT/SDK materialization, native C compiler/linker selection, tar/zip tooling, GitHub runner image identity, manifest schema, and release-level provenance aggregation. Distinguish tools that affect executable/archive bytes from orchestration-only tools.

## Repair direction

Extend the existing release manifest with a compact provenance record gathered during each build. Prefer recording actual resolved versions over duplicating desired versions in another config file. Pin only the tool versions whose mutability creates material release/reproduction risk; do not introduce a full supply-chain attestation framework solely for this debt.

## Exit criteria

- Each published artifact records the resolved Rust compiler/Cargo identity used to build it.
- Windows artifacts record the resolved cargo-xwin identity and relevant Windows SDK/CRT build context available from the build path.
- Relevant native compiler/linker and archive-tool identities are recorded where they can affect distributed bytes.
- Provenance survives with the release independently of CI log retention.
- A maintainer can compare two historical artifacts' materially relevant build environments from release metadata alone.
