---
summary: "Records duplicated release-source cache lifecycle authority between runtime retention logic and GitHub Actions cache orchestration."
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

# Release Source-Retention CI Cache Lifecycle Coupling

**Priority:** Low

## Root cause

The release source-retention implementation owns the cache schema and default dependency-cache layout, while GitHub Actions independently encodes the retention cache path and schema-bearing cache key. CI also keys the entire retained-source snapshot from `deps/pawnio.ts` rather than consuming a cache identity produced by the retention authority itself. The runtime and CI layers therefore contain facts that must evolve together even though only the runtime understands the retained representation and immutable source identities.

## Optimization dimension

Primary: architecture and change efficiency. Secondary: operations and lifecycle efficiency, build and development-feedback efficiency.

## Current cost

A future retention-schema, cache-layout, or source-plan change requires coordinated edits across runtime code and workflow YAML. Missing one side can restore stale data under an obsolete key or unnecessarily cold-start the retention cache. Because source entries are individually immutable and keyed by repository plus exact commit, invalidating the whole CI snapshot when the dependency declaration changes also gives up avoidable reuse of unchanged retained entries and can reintroduce upstream Git acquisition on release paths that already possess useful retained material.

## Evidence

The retention layer currently declares its own cache schema and derives its default cache root beneath `CCLOVER_MON_DEPS_CACHE` / `~/.cache/cclover-mon/deps`. The release workflow separately names `~/.cache/cclover-mon/deps/release-sources` and embeds `release-sources-v1-` in the GitHub Actions cache key, with `hashFiles('deps/pawnio.ts')` controlling snapshot identity. These two definitions describe one lifecycle contract but are maintained independently.

## Reachable better state

The retention authority exposes one stable machine-consumable cache contract or fingerprint covering representation schema and current source-plan identity. CI consumes that contract instead of restating schema or layout semantics. CI restore behavior should preserve reuse of immutable retained entries across compatible source-plan changes while still preventing incompatible representation versions from mixing.

## Governing constraint

Release CI must not independently define semantic retention-cache schema, layout, or compatibility rules that already belong to the source-retention authority; compatible immutable retained entries should remain reusable across source-plan evolution without weakening fail-closed verification.

## Scope discovery

Trace the retention cache contract from `tools/release/source-retention.ts` through `release.ts`, the PawnIO dependency/source declarations, and `.github/workflows/release.yml`. Include every CI restore/save key, path, schema/version token, source-plan fingerprint, and cache compatibility decision. Check for other workflows or local automation that persist the same retention cache and ensure they consume the same authority.

## Repair direction

Add the smallest machine-readable cache identity seam to the existing release tooling and make CI derive its key/path compatibility from that output. Prefer exact current-state keys plus compatible-prefix restore or an equivalent immutable-entry-preserving strategy. Do not introduce a general cache service, remote dependency mirror, or separate package manager.

## Exit criteria

- Retention representation schema and cache-layout compatibility are defined once by release tooling rather than duplicated in workflow YAML.
- CI obtains its cache identity from that authority or from a generated value mechanically tied to it.
- A compatible PawnIO/source-plan update can reuse unchanged immutable retained entries instead of necessarily starting from an empty snapshot.
- An incompatible retention representation version cannot be restored as compatible data.
- Cache misses still fetch and verify exact pinned source identities, while corrupt retained entries continue to fail closed.
- Local release behavior remains independent of GitHub Actions-specific cache mechanics.

