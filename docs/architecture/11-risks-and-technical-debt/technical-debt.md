---
summary: "Indexes current architecture-level technical debt by root cause and priority."
viewpoint: assurance
concerns:
  - maintainability
  - portability
  - performance
  - security
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
---

# Technical Debt

Technical debt here means a current system property that creates meaningful avoidable cost, risk, resource use, degraded experience, or unnecessary work when a materially better and reasonably achievable state exists. Each root cause has one document; symptoms and code smells are evidence, not separate debts. External/runtime uncertainty that is not itself an avoidable system cost belongs in [Architecture Risks](risks.md).

Presence is the state: every document in `technical-debt/` represents debt that currently exists. Do not add `Status` fields or retain resolved debt documents. When a debt is resolved, remove its index entry and delete its document; Git history preserves the former problem and its resolution. Durable constraints or residual runtime uncertainty must first be moved to the appropriate architecture View or risk document.

Current architecture-level technical debt:

- [Performance diagnostic composition authority](technical-debt/performance-diagnostic-composition-authority.md) — Low-medium priority; the current repair is converging production/performance prerequisite authority, but closure remains pending until the full production-context runtime proof is completed.
- [Scene positional invalidation damage amplification](technical-debt/scene-positional-invalidation-damage-amplification.md) — Low-medium priority; dynamic insertion or removal is conservatively matched by ordinal position, so a local collection change can damage much of the following Scene even when most primitives are unchanged.
- [Vendored hwmon warning isolation](technical-debt/vendored-hwmon-warning-isolation.md) — Low-medium priority; accepted upstream hwmon warnings are emitted in the same Windows cross-build output as project-owned native diagnostics, repeatedly obscuring the validation result and increasing output-handling work.
- [Linux release runtime ABI authority](technical-debt/linux-release-runtime-abi-authority.md) — Medium priority; published Linux artifacts lack one system-wide runtime ABI baseline, so build-environment changes can silently raise host requirements.
- [libbpf runtime diagnostics ownership](technical-debt/libbpf-runtime-diagnostics-ownership.md) — Low-medium priority; libbpf can still emit raw stderr diagnostics outside the product's typed diagnostic and logging policy.
- [Privileged runtime validation execution authority](technical-debt/privileged-runtime-validation-execution-authority.md) — Medium priority; validation profiles declare host/privilege requirements centrally, but individual proof scripts and host adapters still own how elevation is actually obtained, repeatedly forcing manual handoff during automated validation.
- [Graphical text font metrics contract](technical-debt/graphical-text-font-metrics-contract.md) — Low-medium priority; the bounded-text contract is implemented and validated on Linux/Web, but its GDI production-font test still needs real-Windows execution before the debt can be closed.
- [Windows AMD Family 10h upstream authority](technical-debt/windows-amd-family10-upstream-authority.md) — Low-medium priority; Family 10h still retains a second temperature-algorithm authority because the pinned official PawnIO transport cannot execute upstream Erratum 319 validation.
- [Windows NDU ABI fixture coverage](technical-debt/windows-ndu-abi-fixture-coverage.md) — Medium-high priority; the undocumented NDU ABI still lacks a representative multi-version raw fixture corpus.
- [Windows NDU end-to-end acceptance](technical-debt/windows-ndu-end-to-end-acceptance.md) — High priority; real process-to-interface NDU attribution still lacks a repeatable end-to-end acceptance harness.
- [Windows server SCM lifecycle validation](technical-debt/windows-server-scm-lifecycle-validation.md) — Medium-high priority; the server service adapter cross-builds on both Windows architectures but lacks real-SCM start/readiness/stop acceptance coverage.
