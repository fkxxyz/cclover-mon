---
summary: "Records that privileged runtime validation declares host/privilege requirements centrally but still leaves actual elevation policy inside individual smoke scripts."
viewpoint: assurance
concerns:
  - maintainability
  - architecture-coherence
  - security
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
---

# Privileged Runtime Validation Execution Authority

**Priority:** Low-medium

## Root cause

`validate.ts` owns validation-profile metadata, including required host and privilege, but privileged execution is not fully consumed by one execution authority. Individual runtime-smoke scripts can still decide how to obtain elevation themselves, so the declared profile contract and the mechanism that actually satisfies it are separate maintained authorities.

## Primary cost dimension

Maintenance and validation reliability.

## Current cost

Maintainers must know which privileged profiles require manual credentials or a host-specific bridge and must trust each smoke script to implement elevation consistently and safely. Adding another privileged runtime proof risks copying `sudo`, administrator bridge, environment forwarding, and failure-reporting policy into another script. A profile can correctly declare `privilege: elevated` while ordinary automated execution still lacks a general mechanism that guarantees the requirement is satisfied.

## Evidence

The Linux `linux-ebpf-runtime` profile declares elevated privilege in `validate.ts`, but its scalar-fallback smoke obtains elevation by invoking `sudo` when the process is not already root. Windows elevated runtime validation uses a repository-owned Windows host bridge. These mechanisms are valid locally, but they demonstrate that privilege execution policy is distributed across profile metadata, host adapters, and individual validation scripts rather than converging at one execution boundary.

## Cost mechanism

Privilege requirement is represented twice: once declaratively in the validation plan and again procedurally in the script or host mechanism that performs the run. New privileged profiles can therefore diverge in prompting, environment handling, command composition, CI suitability, and failure semantics. The metadata cannot by itself guarantee that the declared execution contract is honored.

## Reachable better state

Make validation execution consume host/privilege metadata through one bounded authority for each supported execution environment. Smoke scripts should describe the proof they perform and assume the requested execution context, rather than independently deciding how to become root/administrator. Reuse the existing Windows host-bridge pattern where appropriate, but do not build a generic remote-execution framework unless more environments require it.

## Governing constraint

Validation profiles own execution requirements; individual proof scripts should not independently redefine host or privilege-acquisition policy when that policy is already declared by the profile.

## Scope discovery

Review every validation profile marked elevated or requiring a non-local host, along with `validate.ts`, `windows-validate.ts`, Linux runtime smokes, Windows runtime smokes, CI workflow entry points, and future capability/root/admin proofs. Identify duplicated decisions about elevation, environment preservation, working-directory mapping, credential prompting, and failure reporting. Keep proof-specific workload logic inside the smoke itself.

## Repair direction

Introduce only the smallest execution layer needed to consume existing profile metadata. For local Linux this may be a bounded elevated-command adapter; for Windows it may remain the existing host bridge. Prefer explicit, auditable command boundaries and preserve least privilege. Avoid hiding arbitrary command execution behind a broad abstraction or moving proof semantics out of the validation scripts.

## Exit criteria

- Every privileged validation profile's declared host/privilege metadata is consumed by an owned execution path rather than reimplemented ad hoc inside the proof script.
- Proof scripts no longer decide independently whether or how to invoke `sudo`, administrator bridges, or equivalent elevation mechanisms.
- Linux and Windows privileged validation retain explicit least-privilege and environment/working-directory behavior.
- Adding a new privileged profile does not require inventing a new elevation convention.
- Ordinary non-privileged profiles remain simple and do not inherit unnecessary privilege machinery.
