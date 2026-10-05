---
summary: "Records that published Linux artifacts lack one system-wide runtime ABI baseline that prevents build-environment changes from silently raising host requirements."
viewpoint: assurance
concerns:
  - portability
  - maintainability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
---

# Linux Release Runtime ABI Authority

**Priority:** Medium

## Root cause

Published Linux artifacts do not have one explicit, executable system-wide runtime ABI baseline. The release process builds on a mutable hosted Linux environment, so glibc and other dynamically linked native-library requirements can change when the build environment changes even when product compatibility policy has not changed.

## Primary cost dimension

Portability and release reliability.

## Current cost

Maintainers cannot state from one authority what the oldest supported runtime ABI is for the published Linux artifacts, nor can they rely on release validation to reject an accidental increase. A hosted-runner or native dependency update can therefore produce an artifact that builds, tests, and publishes successfully but no longer starts on Linux installations that previously ran the product.

The libbpf dependency is now separately governed by an explicit `libbpf.so.1` / `LIBBPF_1.0.0` contract and ELF check, but that local authority does not cover glibc or the rest of the dynamically linked Linux runtime surface.

## Evidence

The Linux release artifact is dynamically linked and depends on host libraries including glibc and native desktop/runtime libraries. Release CI currently derives those requirements from its build environment rather than from a declared product-wide minimum runtime ABI. During the libbpf compatibility repair, the repository needed a dedicated ELF guard precisely because a newer development or CI environment can otherwise raise imported symbol requirements without a deliberate compatibility decision.

## Cost mechanism

The build environment acts as an implicit compatibility authority. Because that environment can advance independently of product policy, generated ELF requirements can drift forward without any source-level change that clearly represents a support decision. Compatibility is therefore partly determined by infrastructure state rather than by an owned product contract.

## Reachable better state

Choose one explicit Linux runtime compatibility baseline for published artifacts and make the release pipeline enforce it. The enforcement may use a pinned/controlled build environment, artifact-level ABI inspection, or a combination, but it should prevent a mutable runner from silently raising runtime requirements. Keep library-specific authorities such as the libbpf contract only where they protect distinct semantics; do not create a manually maintained distro/version matrix unless product support policy actually requires one.

## Governing constraint

A published Linux artifact may raise its minimum runtime ABI requirements only through an explicit compatibility-policy change, never merely because the release build environment or native dependency set became newer.

## Scope discovery

Inspect every dynamically linked requirement of both published Linux products: glibc symbol versions, ELF interpreter requirements, native shared-library SONAMEs, desktop/runtime libraries for the interactive artifact, and libbpf for eBPF-enabled products. Inspect the release runner/build environment and packaging verification paths that can influence those requirements. Distinguish product-wide ABI policy from library-specific contracts that already have stronger local authorities.

## Repair direction

First define the minimum runtime baseline the project is prepared to support. Then enforce the smallest sufficient artifact/build constraint that makes accidental increases fail release validation. Prefer deriving evidence from final ELF artifacts and controlled build provenance over duplicating dependency-version facts in prose. Avoid broad compatibility frameworks or per-distribution test matrices unless concrete support requirements justify them.

## Exit criteria

- One architecture/deployment authority states the minimum supported Linux runtime ABI policy for published artifacts.
- Both interactive and server Linux release artifacts are mechanically checked against that policy, or are produced in a build environment that makes exceeding it impossible and is itself verified as release provenance.
- A representative increase in glibc or another governed native runtime requirement fails release validation before publication.
- Mutable hosted-runner upgrades cannot silently redefine the compatibility floor.
- Existing library-specific contracts, including libbpf, either derive from the product-wide policy or retain an explicit distinct responsibility without contradictory baselines.
