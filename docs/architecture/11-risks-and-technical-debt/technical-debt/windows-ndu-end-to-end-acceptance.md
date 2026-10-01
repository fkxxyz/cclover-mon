---
summary: "Records the missing deterministic Windows acceptance harness for proving real NDU process attribution end to end."
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

# Windows NDU End-to-End Acceptance

**Priority:** High

## Root cause

Windows NDU parser logic, PID continuity policy, interface identity canonicalization, and cumulative-counter adaptation are deterministically testable, but the critical OS-to-NDU process-attribution behavior still depends on live Windows state and has no repeatable acceptance harness that proves a known process produces nonzero attributed traffic through the production path.

## Evidence

Windows-host tests currently exercise the NDU parser and attribution policy successfully, and the production `network-attribution` probe can open/query NDU without parser failure. During controlled runtime checks, short NDU intervals often returned only non-process attribution records, while sustained traffic from a known `curl.exe` process did not reliably produce a directly verifiable nonzero row for that PID. This means existing deterministic tests prove the wrapper and policy layers, but not the complete path from a chosen Windows process through NDU accounting to cclover-mon's process × interface output.

## Governing constraint

A claimed Windows per-process network attribution capability must have a repeatable acceptance path that proves a known process can generate controlled nonzero network traffic and that the production collector reports that same process on the canonical network identity with nonzero directional usage. Manual observation may supplement this evidence but must not remain the only way to establish correctness.

## Scope discovery

Review the Windows-only validation path from launching a known traffic-generating process, through NDU session start/query timing and privilege requirements, PID identity capture, IfLuid-to-NetworkId canonicalization, cumulative counter projection, core rate derivation, and final diagnostic or CLI output. Include any Windows runner/provisioning needed to make the test reproducible. Do not replace this acceptance path with mocks of NDU output; lower-level parser and policy tests already cover simulated data.

## Maintenance consequence

Changes to NDU session lifecycle, privilege handling, sampling order, PID continuity, interface identity mapping, or Windows-version compatibility can pass unit tests and cross-compilation while breaking real attribution. Without a repeatable end-to-end harness, maintainers must recreate ad hoc traffic experiments and interpret live machine behavior manually, so regressions are expensive to verify and easy to miss.

## Repair direction

Add the smallest Windows-only acceptance harness that launches a known helper process, records its process identity, produces controlled traffic for long enough to cross NDU sampling boundaries, runs the production collector path, and asserts a nonzero attribution for that process on the expected canonical interface. Prefer a deterministic local or otherwise controlled traffic source over dependence on public Internet timing. Keep NDU internals behind the existing wrapper and avoid building a second diagnostic collector solely for the test.

If investigation establishes that NDU cannot reliably expose such real-time per-process attribution on supported systems, record that platform capability limit explicitly and reassess the production data-source decision instead of weakening the acceptance criterion into a non-process or zero-byte success.

## Exit criteria

- A Windows-only automated or scripted acceptance test launches a known process and generates controlled nonzero network traffic through it.
- The test captures the process identity and verifies that the production cclover-mon path reports nonzero process × interface attribution for that same process.
- The reported interface resolves through the same canonical `NetworkId` authority as ordinary Windows interface collection.
- The harness is repeatable on the project's supported Windows validation environment without manual interpretation of output.
- Existing parser and policy unit tests remain deterministic lower-level coverage rather than substitutes for the real acceptance path.
- If NDU is proven incapable of satisfying these criteria on supported Windows builds, the debt is closed only by recording the capability boundary and revisiting the NDU source decision.
