---
summary: "Records that the validation plan is mirrored by hand-maintained literals in its tests, so new test files and profile edits need synchronized edits and test-file completeness is unproven."
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

# Validation Plan Test Mirroring

**Priority:** Low-medium

## Root cause

`validate.ts` is the validation-plan authority, but `validate.test.ts` restates its contents as literals: the exact command list of most profiles and the enumerated test files of the fast profile. Which test files run is a manually maintained list, and no check proves that the list covers every `*.test.ts` file in the repository.

## Evidence

Adding one test file and one profile required matching edits in both files; the fast profile's file list had to be extended in `validate.ts` and again in the expected literal, and a first edit that missed it was caught only because the mirrored literal disagreed. Had the new file been omitted from both, no check would fail and the file would never run. This is observed from one change; the number of existing test files absent from the list is unknown.

## Governing constraint

The validation plan has one source of truth. Tests of the plan assert invariants that protect real contracts (per-invocation `XWIN_ARCH`, `--locked`, a profile's required host and privilege, which profiles must not contain a step) rather than restating every command. Test-file execution is derived from, or proven complete against, the files that exist.

## Scope discovery

Compare the fast profile's enumerated files with every `*.test.ts` in the repository, and classify each assertion in `validate.test.ts` as an invariant or a restated command. Include `windows-validate.test.ts` and the release tests only where they mirror the plan. Exclude tests of release planning and packaging logic.

## Maintenance consequence

Every plan edit costs two coordinated changes, a test that only restates a literal gives little protection, and a forgotten test file silently reduces proof.

## Repair direction

Derive the fast-profile test list by discovery, or add one test asserting the set of `*.test.ts` files equals the set run. Reduce restated command lists to invariant assertions, keeping an exact list only where the order or content is itself the contract.

## Exit criteria

- A new `*.test.ts` file cannot exist without being run, and this is enforced automatically.
- `validate.test.ts` contains no assertion that merely duplicates a profile's command list without protecting a named contract.
- Adding a profile step requires editing `validate.ts` only unless it changes an invariant.
