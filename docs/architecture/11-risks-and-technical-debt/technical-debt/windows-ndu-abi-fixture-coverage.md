---
summary: "Records the missing multi-version fixture corpus for validating the undocumented Windows NDU attribution ABI."
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
    - platform
    - whole-system
---

# Windows NDU ABI Fixture Coverage

**Priority:** Medium-high

## Root cause

Windows per-process network attribution depends on the undocumented `NduIoDevice` ABI. The production wrapper isolates and bounds-checks that ABI, but compatibility knowledge is currently supported by a narrow set of observed layouts rather than a corpus of raw snapshots from representative supported Windows builds.

## Evidence

Implementation work against Windows 11 build 26100 exposed layout assumptions that were not visible from the initial process-record sample: non-process attribution kinds do not share the process-record nested-interface layout, and zero-count lists must not require their otherwise-unused self-relative offsets to be valid. Those cases required parser hardening after real runtime observation. Current deterministic parser tests cover the understood layouts and fail-closed rules, but they do not establish compatibility across multiple Windows builds or future ABI variants.

## Governing constraint

All undocumented NDU wire assumptions must remain isolated in the Windows-private NDU wrapper and must be validated against reproducible raw compatibility evidence. Unknown layouts must fail closed; collector policy must never infer offsets or reinterpret private record kinds outside the wrapper.

## Scope discovery

Review the NDU wrapper's device IOCTLs, envelope fields, record strides, kind-specific layouts, self-relative pointers, count semantics, and parser tests. Include raw snapshot evidence collected from every Windows build intentionally covered by the project, plus any new layout encountered after Windows updates. Exclude higher-level PID continuity, interface identity, rate derivation, and presentation policy unless a fixture reveals that the raw ABI itself changed those typed outputs.

## Maintenance consequence

A Windows update can change or extend the private ABI while all higher-level unit tests and cross-compilation still pass. Without a representative fixture corpus, maintainers must rediscover compatibility behavior interactively on real systems, increasing verification cost and making subtle parser regressions or unsupported layouts harder to distinguish from ordinary NDU data absence.

## Repair direction

Build a small versioned corpus of raw NDU snapshots captured from representative supported Windows builds. Store only the minimum binary evidence and metadata needed to reproduce parser behavior, including the Windows build and the observed record scenario. Replay the corpus through deterministic parser contract tests. Add a new fixture before extending parsing for any newly observed layout; do not generalize private structures beyond observed evidence.

## Exit criteria

- A checked-in fixture corpus contains representative raw NDU snapshots from the Windows builds the project intentionally claims compatibility with.
- Each fixture records sufficient provenance to identify the originating Windows build and observed scenario.
- Deterministic parser tests replay the complete corpus and verify typed process/interface output or intentional opaque handling for unsupported kinds.
- Newly observed ABI layouts are added to the corpus before parser support is extended.
- Scope discovery confirms that all private NDU offsets, strides, kinds, and self-relative pointer rules remain owned by one wrapper and no duplicated ABI authority exists elsewhere.
