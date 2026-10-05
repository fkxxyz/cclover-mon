---
summary: "Records release-time availability coupling between corresponding-source fulfillment and third-party upstream Git repositories."
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

# Release Corresponding-Source Upstream Availability Coupling

**Priority:** Medium

## Root cause

The release pipeline has immutable corresponding-source identities for redistributed PawnIO payloads, but materializes those sources by cloning the third-party upstream repositories during each release. Release availability therefore still depends on the live availability of PawnIO, PawnIO.Modules, and recursively required source-submodule repositories even when the exact required commits are already known and unchanged.

## Optimization dimension

Primary: operations and lifecycle efficiency. Secondary: reliability and recovery efficiency, build and development-feedback efficiency.

## Current cost

Every Windows release pays avoidable external-network and third-party-service availability risk before source fulfillment can be produced. A temporary upstream outage, repository removal, rate limit, DNS/network failure, or inaccessible historical submodule URL can block an otherwise reproducible release even though the required top-level and gitlink commits are immutable. Maintainers also cannot reproduce the full release offline from retained project-controlled inputs.

## Evidence

The release plan pins exact corresponding-source commits for PawnIO and PawnIO.Modules, and the source-fulfillment path verifies those commits plus recursive gitlink identities before producing deterministic source assets. However, generation still begins from live upstream Git clones. PawnIO 2.2.0 additionally requires the PawnPP source submodule, so release success depends on more than one external repository remaining reachable.

## Reachable better state

Retain verified immutable corresponding-source material under project-controlled release infrastructure or a content-addressed user/CI cache keyed by repository plus exact commit. Source fulfillment should consume retained verified material when present and use upstream network access only to populate a missing immutable object. The retained representation must preserve recursive submodule source and provenance without vendoring third-party source into the main repository unless that later becomes the simpler ownership choice.

## Governing constraint

Once a third-party source identity is pinned and verified, an ordinary release of that unchanged dependency version must not require live third-party repository availability to reconstruct its corresponding-source fulfillment asset.

## Scope discovery

Trace every release-time source input beginning at `ReleaseCompanionAssetPlan`: top-level repositories, pinned commits, recursive `.gitmodules` URLs, gitlink commits, CI source-fulfillment jobs, local release reproduction, and any retained artifact/cache boundary. Include cache integrity, cache miss behavior, provenance preservation, cleanup/retention policy, and offline reconstruction of all companion source assets.

## Repair direction

Add the smallest immutable retention layer around corresponding-source material. Prefer content-addressed verified storage with fail-closed digest/commit checks and an upstream fetch path only for cache misses. Do not build a general dependency mirror or package manager; solve only the release-critical corresponding-source inputs governed by the release plan.

## Exit criteria

- A previously populated source cache or retained source artifact can regenerate every declared companion source asset with external network access disabled.
- Top-level source commits and recursive submodule gitlink commits remain mechanically verified before use.
- Cache corruption or identity mismatch fails closed rather than silently falling back to unverified bytes.
- A cache miss may fetch from upstream, verify the declared immutable identities, and populate the retained representation for later releases.
- Release documentation and CI use the same retention/materialization authority rather than separate online/offline paths.
- The resulting companion assets remain deterministic and preserve the same durable source and producer provenance required by release verification.
