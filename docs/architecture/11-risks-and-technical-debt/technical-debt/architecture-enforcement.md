---
summary: "Tracks architecture and quality rules that are documented but not mechanically enforced."
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

# Architecture Enforcement

## Status

Active — P1.

## Problem

Several important architecture and validation rules exist primarily as prose and developer workflow rather than executable constraints. The current single-crate structure and repository automation do not reliably fail when dependency direction or required validation is violated.

## Evidence

Rules such as `core` not depending on `platform`, presentation remaining renderer-neutral, and native protocols staying behind platform boundaries are documented but not mechanically checked. Repository validation commands are documented, but there is no repository CI/workflow that consistently executes the full applicable quality gate on changes. Cross-layer contracts have limited automated coverage compared with local parser/derivation tests.

## Maintenance impact

As contributors, platforms, native integrations, and frontends increase, architecture drift can compile successfully and survive until review or runtime testing. Manual quality gates are also easiest to skip on seemingly small changes.

## Governing constraint

Important architecture invariants and repeatable validation steps should fail mechanically wherever the cost of enforcement is reasonable. Documentation remains authoritative for intent, while automation protects the invariant from accidental erosion.

## Resolution direction

Add lightweight dependency/boundary checks and automated validation suitable for the repository. Prefer targeted checks over splitting crates solely for enforcement. Add contract-level tests where a documented boundary has meaningful behavior not covered by local unit tests.

## Exit criteria

Critical dependency-direction violations and required repository validation are automatically detectable, and normal contribution paths exercise the quality gate without relying on developer memory alone.
