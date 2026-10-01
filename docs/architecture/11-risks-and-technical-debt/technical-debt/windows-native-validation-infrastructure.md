---
summary: "Records the missing repository-owned execution/provisioning path for reproducible Windows-native validation from Linux or WSL development environments."
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

**Priority:** Medium-high

## Root cause

The repository defines native Windows validation profiles, but invoking them from the project's common Linux/WSL development environment still depends on machine-local host-interoperability and privilege setup that is not owned, discovered, or provisioned by the repository. Cross-build validation is therefore reproducible while real Windows execution remains environment-specific.

## Evidence

`validate.ts` provides `windows-native` for deterministic tests on a Windows host and `windows-etw-runtime` for the real ETW disk-attribution smoke. During ETW implementation, both x86 and x64 cross-builds were reproducible, but the active Linux-side automation environment had neither an available `cmd`/PowerShell interop command nor a mounted Windows host path, so the freshly built executable could not be launched on the host from that environment. Previously provisioned administrator bridges are machine-local state rather than a repository contract and are not discoverable from a clean session.

## Governing constraint

Any Windows-native validation that the project relies on should have one reproducible invocation contract that tells a fresh maintainer or agent how to reach a Windows host, how ordinary and elevated execution differ, and how repository paths/artifacts are made visible to the host. Validation profiles remain the authority for what is executed; host transport must not duplicate their command lists.

## Scope discovery

Review `validate.ts` Windows-native profiles, GitHub/CI Windows runners, WSL-to-Windows invocation helpers, administrator/scheduled-task bridges, path translation for repository artifacts, and maintenance documentation for runtime validation. Include Windows-native acceptance work for ETW, NDU, hardware telemetry, and native desktop behavior. Exclude metric-specific semantic assertions, which belong in their feature-specific debt records.

## Maintenance consequence

Native Windows regressions can require session-specific setup knowledge before tests can even run. New maintainers or agents may repeat bridge discovery, fall back to manual clicking, or incorrectly treat cross-compilation as runtime evidence. Privileged collectors are especially costly because a missing elevation path can make otherwise automated validation appear unavailable.

## Repair direction

Define the smallest repository-owned host-execution contract that can invoke an existing `validate.ts` profile on a Windows host from the supported development environment. Reuse native Windows runners directly when already on Windows. For WSL development, document or provide one stable bridge entry with explicit ordinary/elevated modes and deterministic path translation. Keep privilege provisioning outside metric collectors and avoid a general remote-execution framework.

## Exit criteria

- A clean development session can discover one documented command/path for ordinary Windows-host execution and one for elevated execution when required.
- The bridge invokes existing `validate.ts` profiles rather than maintaining duplicate validation command lists.
- Repository/worktree paths are translated deterministically and work for both the main tree and temporary worktrees.
- The path works without interactive UAC prompts after its documented one-time provisioning step, where local policy permits that setup.
- A fresh-session test demonstrates `windows-native` and one privileged native profile can be launched without relying on remembered machine-local details.
- Scope discovery confirms Windows-native feature validation no longer has separate ad hoc host-launch mechanisms.
