---
summary: "Records the remaining fresh-session acceptance gap for the repository-owned Windows native validation bridge."
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

# Windows Native Validation Infrastructure

**Priority:** Low-medium

## Root cause

The repository now owns Windows-host validation discovery, path translation, ordinary execution, privilege routing, and one-time elevated Scheduled Task provisioning, but that complete bridge has not yet been exercised from a clean supported WSL/Linux-to-Windows session after introduction. The remaining debt is acceptance evidence for the execution contract rather than missing structure.

## Evidence

`validate.ts` owns validation commands plus required host and privilege metadata. `windows-validate.ts` resolves the current repository/worktree and delegates Windows-host execution through `tools/windows-validation/host.ts`; the Windows runner invokes only `bun validate.ts <profile>`. Deterministic tests cover profile privilege authority, WSL path translation, configured path-prefix translation, arbitrary worktree suffixes, and rejection of non-Windows profiles.

The active implementation session is a native Linux environment mounting a WSL filesystem at `/run/media/...`; it has neither WSL interop nor a configured Windows PowerShell transport. `bun windows-validate.ts doctor` therefore correctly reports that host mapping/transport is unavailable, but this environment cannot supply the final real-Windows ordinary/elevated execution evidence.

## Governing constraint

Any Windows-native validation that the project relies on has one repository-owned invocation contract. `validate.ts` remains authoritative for what is executed and what privilege it requires; the host bridge owns only how that profile reaches Windows. Machine-local configuration may describe PowerShell reachability and path mappings but must not contain validation commands or privilege policy.

## Scope discovery

Review `validate.ts` execution metadata, `windows-validate.ts`, `tools/windows-validation/`, GitHub/CI Windows runners, WSL-to-Windows path translation, administrator Scheduled Task provisioning, and `docs/maintenance/windows-native-validation.md`. Include Windows-native acceptance work for ETW, NDU, hardware telemetry, and native desktop behavior. Exclude metric-specific semantic assertions, which belong in their feature-specific debt records.

## Maintenance consequence

Until a clean-session acceptance run is recorded, deterministic tests can prove the bridge's policy and path logic but cannot prove that current Windows PowerShell, Scheduled Task, UNC/worktree access, Bun/Cargo execution, and exit-code propagation cooperate on the supported development host. A fresh maintainer may still encounter a machine-integration defect that repository-only tests cannot expose.

## Repair direction

From a supported WSL/Linux-to-Windows development session, provision the bridge once with `bun windows-validate.ts install`, then start a fresh shell and run `doctor`, `windows-native`, and one elevated profile through `windows-validate.ts`. Fix only defects exposed at this host-execution boundary; do not add another transport or duplicate validation commands unless the supported development environment requires it.

## Exit criteria

- A fresh development session passes `bun windows-validate.ts doctor` without remembered setup steps beyond documented one-time provisioning.
- `bun windows-validate.ts windows-native` executes the ordinary Windows-host profile from the current checkout/worktree and propagates its result.
- `bun windows-validate.ts windows-etw-runtime` reaches the provisioned elevated path without an interactive UAC prompt after installation and propagates its result.
- The same path works from a temporary Git worktree, proving path translation is not tied to the main checkout.
- Scope discovery confirms Windows-native feature validation no longer has separate ad hoc host-launch mechanisms.
