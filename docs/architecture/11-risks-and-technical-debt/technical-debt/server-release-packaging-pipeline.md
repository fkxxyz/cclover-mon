---
summary: "Records the missing first-class release and packaging pipeline for the cclover-mon-server product artifact."
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

# Server Release And Packaging Pipeline

**Priority:** Medium-high

## Root cause

`cclover-mon-server` is now a distinct product artifact with its own runtime, validation profile, Linux service files, and Windows service installation support, but repository release automation does not yet define it as a first-class published artifact alongside `cclover-mon`.

## Optimization dimension

Primary: operations and lifecycle efficiency. Secondary: artifact and distribution efficiency, test and verification efficiency.

## Current cost

A maintainer preparing a release must remember to build, name, package, and ship the server executable and its platform service assets separately from the interactive product. That manual product-matrix knowledge is recurrent release work and creates avoidable risk that a release publishes only `cclover-mon`, omits service assets, or ships mismatched server binaries and packaging metadata.

## Evidence

The repository now has deterministic Linux server validation, Windows cross-build coverage for both supported architectures, `packaging/server/systemd/`, and `packaging/server/windows/`. These establish product and deployment intent, but there is no repository-owned release flow that emits the server artifact and corresponding service assets as an explicit release deliverable for each supported platform.

## Reachable better state

Extend the repository's eventual release authority so product artifacts are declared once and the pipeline builds and publishes both interactive and server deliverables for supported target/architecture combinations. Server packaging should bundle or otherwise publish the matching systemd or Windows service assets without requiring hand-assembled release steps.

## Governing constraint

Every supported product artifact must be represented explicitly in release automation. Adding or removing a product or target must update one release authority, and published server binaries and service assets must come from the same source revision and validation scope.

## Scope discovery

Review release workflow definitions, Cargo dist/release profile usage, artifact naming, supported Linux/Windows target matrix, checksums/signatures if present, systemd unit/drop-in delivery, Windows service installation assets, documentation that names release outputs, and any downstream packaging scripts. Do not fold ordinary development validation into release packaging unless release correctness depends on it.

## Repair direction

When release automation is introduced or next modified, add `cclover-mon-server` to the same declarative product matrix as `cclover-mon` rather than adding isolated one-off build commands. Keep server service assets versioned with the binary and make artifact naming distinguish product and target unambiguously.

## Exit criteria

- Release automation has one explicit authority listing both `cclover-mon` and `cclover-mon-server` products.
- Every supported release target produces the intended server executable automatically.
- Linux and Windows server service assets are published or packaged through the same release flow as the corresponding binary.
- Artifact names identify product, platform, and architecture without manual renaming.
- A release cannot silently omit the server product while the interactive product succeeds.
- Release documentation describes generated server outputs rather than requiring undocumented manual assembly.
