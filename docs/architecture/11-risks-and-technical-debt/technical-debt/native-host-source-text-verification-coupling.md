---
summary: "Records native-host verification that remains coupled to concrete C source text rather than the cheapest stable contract."
viewpoint: assurance
concerns:
  - maintainability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - native-bridge
---

# Native Host Source-Text Verification Coupling

**Priority:** Low

## Root cause

Some native desktop-host contracts are still verified by searching for concrete C function names, expressions, and source-text structure. Those assertions can encode useful architectural intent, but several currently use implementation shape as a proxy for behavioral or ownership facts that have a cheaper and more stable proof layer.

## Optimization dimension

Primary: test and verification efficiency. Secondary: architecture and change efficiency, cognitive and context efficiency.

## Current cost

Semantics-preserving native-host refactors can require unrelated test maintenance because renaming a helper, moving responsibility to another translation unit, or replacing direct state access with a module contract changes the searched text even when the governed behavior remains unchanged. This adds avoidable edit-feedback work and makes maintainers distinguish genuine contract regressions from source-shape churn after otherwise local native refactors.

## Evidence

During the native C translation-unit boundary refactor, Linux native code compiled successfully and preserved the intended ownership split, but two `tests/desktop_host_boundary.rs` assertions failed because they still expected the former textual-inclusion-era helper names and direct `previous_buffer` expressions. The tests had to be rewritten to follow the new function names and ownership path even though the underlying incremental-rendering and surface-reset contracts were intentionally preserved. Other assertions in the same boundary suite still inspect concrete source text because some static architecture properties genuinely require source inspection.

## Reachable better state

Each native-host contract is proved at the cheapest stable layer that actually establishes it: deterministic behavior and state transitions through compiled policy or contract tests, enforceable static architecture through `archgate.ts`, and real compositor or Win32 integration facts through targeted runtime acceptance. Source-text assertions remain only where textual/static structure is itself the contract and no stronger low-cost mechanical boundary already proves it.

## Governing constraint

Native-host verification must not depend on incidental helper names, expression spelling, or file-local implementation shape when the intended invariant can be established reliably through a compiled deterministic seam, an architecture gate, or a targeted integration contract.

## Scope discovery

Review `tests/desktop_host_boundary.rs` and sibling native-host assurance tests. For each source-text assertion, identify the actual invariant, classify it as behavior, static architecture, or OS integration, and locate any existing lower-level policy module, `archgate` rule, compiled boundary test, or runtime profile that can prove the same fact. Keep source inspection where source topology or forbidden dependency presence is the actual invariant.

## Repair direction

Migrate assertions opportunistically when their governed native code next changes. Prefer existing deterministic policy seams and architecture gates; introduce a new compiled seam only when repeated source-text churn demonstrates concrete value. Do not build a general C test framework or simulate complete window systems solely to eliminate textual assertions.

## Exit criteria

- Native-host source-text assertions are limited to invariants whose subject is genuinely static source structure or for which no cheaper stable proof exists.
- Behavioral state-machine and ownership semantics that already have deterministic compiled seams are tested through those seams rather than helper-name or expression matching.
- Architecture constraints with a practical repository-wide static rule are enforced by `archgate.ts` or an equivalent mechanical gate.
- A representative semantics-preserving rename or responsibility move inside a native module does not require updating unrelated behavior assertions solely because source spelling changed.
- Required real Wayland/X11/Win32 integration contracts remain covered at the integration layer rather than being incorrectly replaced by lower-level tests.
