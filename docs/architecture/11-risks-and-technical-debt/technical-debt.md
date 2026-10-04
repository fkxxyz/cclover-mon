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

- [Graphical text font metrics contract](technical-debt/graphical-text-font-metrics-contract.md) — Medium priority; shared bounded text-slot geometry still lacks a renderer-level guarantee that the concrete Web, Cairo, and GDI font realizations satisfy the same advance budget.
- [Native C host textual module boundaries](technical-debt/native-c-host-textual-module-boundaries.md) — Low-medium priority; Linux and Windows host responsibilities are physically separated but still share one C translation unit, so cross-fragment dependencies are not compiler-enforced.
- [Release publication transactionality](technical-debt/release-publication-transactionality.md) — Medium-high priority; locally verified artifacts are still published through a non-transactional remote mutation that can expose partial releases or require manual retry cleanup.
- [Release build provenance](technical-debt/release-build-provenance.md) — Low-medium priority; release manifests identify source revision and artifact hashes but not the resolved compiler/tool environment that produced the bytes.
- [Windows AMD Family 10h upstream authority](technical-debt/windows-amd-family10-upstream-authority.md) — Low-medium priority; Family 10h still retains a second temperature-algorithm authority because the pinned official PawnIO transport cannot execute upstream Erratum 319 validation.
- [Windows NDU ABI fixture coverage](technical-debt/windows-ndu-abi-fixture-coverage.md) — Medium-high priority; the undocumented NDU ABI still lacks a representative multi-version raw fixture corpus.
- [Windows NDU end-to-end acceptance](technical-debt/windows-ndu-end-to-end-acceptance.md) — High priority; real process-to-interface NDU attribution still lacks a repeatable end-to-end acceptance harness.
- [Windows PawnIO redistribution source obligations](technical-debt/windows-pawnio-redistribution-source-obligations.md) — High priority; Windows archives carry licenses and exact provenance but do not yet mechanically close the applicable corresponding-source or source-offer obligations for redistributed PawnIO payloads.
- [Windows server SCM lifecycle validation](technical-debt/windows-server-scm-lifecycle-validation.md) — Medium-high priority; the server service adapter cross-builds on both Windows architectures but lacks real-SCM start/readiness/stop acceptance coverage.
