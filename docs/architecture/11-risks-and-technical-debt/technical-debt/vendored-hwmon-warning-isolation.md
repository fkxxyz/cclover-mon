---
summary: "Records that accepted vendored Linux hwmon warnings pollute Windows native validation output instead of being isolated from project-owned diagnostics."
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
    - native-bridge
---

# Vendored Hwmon Warning Isolation

**Priority:** Low-medium

## Root cause

Windows hardware-telemetry compatibility compiles pinned vendored Linux hwmon sources through project-owned native bridge translation units, and accepted upstream compiler warnings are emitted into the same validation stream as project-owned native diagnostics. The repository documents those upstream warnings as acceptable, but does not isolate their diagnostic policy from code the project owns.

## Primary cost dimension

Build and verification signal quality.

## Current cost

Every affected Windows cross-build prints a repeated block of known warnings. Maintainers and agents must visually distinguish accepted vendor noise from actionable project warnings, increasing output-scanning cost and making a newly introduced native warning easier to miss. The cost recurs across both supported Windows architectures and across test-compile and release-build validation. In the latest composition-authority validation, the warning volume was large enough that the useful end-of-profile result was easier to inspect only after redirecting the complete Windows validation stream to a file and tailing the final section.

## Evidence

Current x86_64 and i686 Windows validation emits repeated warnings from pinned `vendor/linux/hwmon/coretemp.c`, `k10temp.c`, and `k8temp.c`, including sign-comparison, unused-parameter, unused-variable, and unused-function diagnostics. The same known block repeated during both direct cargo-xwin checks and the full Windows validation profile in the latest work, materially inflating output and obscuring the final validation signal. `docs/maintenance/linux-hwmon-coverage.md` explicitly classifies warnings from unchanged vendored Linux C as accepted upstream-source warnings rather than project-owned source warnings, so the noise is known and recurring rather than an unresolved correctness signal.

## Cost mechanism

Vendor and project-owned C share one compiler diagnostic stream without a narrow warning-isolation boundary. Because accepted warnings cannot currently be suppressed only for their ownership scope, the validation output repeatedly carries diagnostics that require no action while project-owned warnings remain semantically important.

## Reachable better state

Apply warning isolation only around unchanged vendored upstream code while preserving strict warnings for project-owned bridge and compatibility code. The exact mechanism may be compiler diagnostic push/pop around vendor inclusion or another ownership-scoped build boundary, but it must not globally weaken native warning policy.

## Governing constraint

Accepted warnings from immutable or digest-checked vendored upstream sources must not obscure actionable warnings from project-owned native code, and suppressing vendor noise must not suppress diagnostics for project-owned code.

## Scope discovery

Inspect every Windows hwmon bridge that compiles vendored Linux source, the compiler flags applied to those translation units, and validation output for both x86_64 and i686 targets. Distinguish pinned upstream code from compatibility shims and bridge code maintained by this repository before choosing any suppression boundary.

## Repair direction

Introduce the smallest ownership-scoped diagnostic boundary that silences only the accepted vendored warnings already covered by source-digest verification. Do not add global `-Wno-*` flags, relax project-owned `-Wall`/`-Wextra` expectations, or fork vendored source merely to make warning output quiet.

## Exit criteria

- Windows x86_64 and i686 cross-validation no longer emits the accepted warning block from unchanged vendored hwmon source.
- A deliberate representative warning in project-owned native bridge code is still surfaced by the same validation path.
- Vendored source remains pinned and digest-checked against its declared upstream authority.
- No warning suppression applies outside the discovered vendored ownership boundary.
