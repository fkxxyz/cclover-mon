---
summary: "Records the lack of atomic, idempotent remote publication for GitHub release assets."
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

# Release Publication Transactionality

**Priority:** Medium-high

## Root cause

The repository now proves the complete local release artifact set before publication, but the final GitHub release mutation is not itself transactional or idempotent. Creating the public release and uploading its assets can fail partway through, and a retry can encounter an already-created release rather than converging automatically to the intended state.

## Optimization dimension

Primary: operations and lifecycle efficiency. Secondary: reliability and recovery efficiency.

## Current cost

A transient GitHub/API/upload failure can leave a visible release with only part of the expected asset set. Maintainers must inspect and repair or delete that remote state manually before retrying, while users can observe or download an incomplete release during the failure window. The local `ReleasePlan` completeness guarantee therefore stops one boundary before the externally visible release state.

## Evidence

Release automation verifies all six declared archives, their common version/source revision, hashes, and archive contents before invoking GitHub publication. The remote step then creates the release and uploads assets in one command without a draft/staging phase, remote asset-set verification, per-tag serialization, or an idempotent retry path.

## Reachable better state

Publish through a draft release scoped to the tag, upload or replace the exact expected asset set, verify the remote names against the locally verified release manifest, and make the release public only after that comparison succeeds. A rerun for the same tag should reuse or repair the draft state and converge instead of requiring manual deletion. Serialize publication attempts per tag so two runs cannot race.

## Governing constraint

A release must not become publicly visible until the remote asset set exactly represents the locally verified release plan, and rerunning publication for the same source tag must converge safely to the same public state.

## Scope discovery

Cover GitHub release creation/update commands, draft/public state, asset upload/replacement semantics, expected asset names from `release-manifest.json`, checksum-manifest delivery, workflow concurrency for a tag, retry behavior after partial upload, and cleanup behavior for failed drafts. Keep local build/packaging completeness in `release.ts`; only remote publication lifecycle belongs here.

## Repair direction

Add the smallest GitHub-specific publication adapter in workflow/script form: ensure or create a draft for the tag, reconcile its assets with the verified manifest, verify the resulting remote set, then publish. Do not introduce a generic release-service abstraction unless a second publication backend creates real pressure.

## Exit criteria

- A failed asset upload cannot expose a partial public release.
- Publication verifies that the remote asset-name set equals the expected release asset set before making the release public.
- Re-running the workflow for the same tag safely converges without manual release deletion.
- Concurrent publication attempts for one tag are serialized or otherwise cannot race into inconsistent remote state.
- Failure recovery leaves either a repairable non-public draft or a complete public release.
