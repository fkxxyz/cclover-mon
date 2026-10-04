---
summary: "Records the incomplete machine-verifiable source-obligation closure for redistributed PawnIO payloads."
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

# Windows PawnIO Redistribution Source Obligations

**Priority:** High

## Root cause

Windows release packaging now carries PawnIO/PawnIO.Modules license material and exact upstream version/source/binary provenance, but it does not yet mechanically establish how every applicable corresponding-source or source-offer obligation is fulfilled for the embedded redistributed payloads. Provenance identifies source; it is not by itself proof that the release satisfies the applicable redistribution mechanism.

## Optimization dimension

Primary: operations and lifecycle efficiency. Secondary: reliability and recovery efficiency.

## Current cost

Every Windows release still requires human legal/compliance reasoning before publication. A maintainer can prove which upstream source/version corresponds to an embedded binary but cannot point to one mechanically enforced release artifact or durable offer that closes the source-delivery obligation required by the governing upstream terms. This creates avoidable release risk and can block confident distribution despite otherwise deterministic packaging.

## Evidence

ADR 009 explicitly makes PawnIO/PawnIO.Modules redistribution a release gate and requires packaging to preserve applicable third-party licenses, notices, corresponding-source or source-offer obligations, and provenance. Current Windows archives contain project GPL terms, MPL/LGPL/PawnIO notices, and `THIRD-PARTY-SOURCES.txt` with pinned upstream source/version/binary hashes. No repository-owned release step currently produces or validates a corresponding-source bundle, valid source offer, or equivalent fulfillment artifact for each embedded payload.

## Reachable better state

Establish the exact redistribution mechanism required for each pinned PawnIO payload from the authoritative upstream terms, then encode that chosen mechanism in release packaging. Where source delivery is required, package or publish the exact corresponding source snapshot/build material alongside the binary release; where a source offer or another mechanism is valid, preserve that evidence in a durable release artifact. Tie fulfillment to the same pinned version/provenance authority and fail closed when required evidence is absent.

## Governing constraint

No Windows release may redistribute an embedded PawnIO/PawnIO.Modules payload unless the repository can mechanically associate that exact payload with both its license/notice material and the concrete fulfillment mechanism required for its applicable source-distribution obligations.

## Scope discovery

Review the authoritative licenses/notices for the pinned PawnIO driver and every embedded PawnIO.Modules binary, the pinned dependency declaration, prepared binary payloads, any source/build inputs required to constitute corresponding source, GitHub source archives versus upstream source snapshots, release archive contents, release notes/assets, and retention/durability requirements for any source offer. Do not infer legal sufficiency solely from a source URL.

## Repair direction

First determine and document the exact obligation per redistributed payload from authoritative terms. Then add the smallest concrete release artifact/check needed to satisfy it and extend `ReleasePlan` packaging verification to fail when that evidence is absent. Avoid building a generic license-management framework.

## Exit criteria

- Each embedded PawnIO/PawnIO.Modules payload has an explicitly documented applicable redistribution/source obligation grounded in authoritative upstream terms.
- Each obligation maps to a concrete source bundle, valid source offer, or other permitted fulfillment mechanism delivered with or durably associated with the release.
- The fulfillment artifact is tied to the same pinned source version/provenance as the embedded binary.
- Release automation fails closed when required source-obligation evidence is absent or mismatched.
- Windows release documentation no longer requires ad hoc interpretation to determine whether PawnIO redistribution is complete.
