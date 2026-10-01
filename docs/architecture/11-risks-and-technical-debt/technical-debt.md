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

Technical debt here means current structure that makes future reasonable change more expensive, risky, or difficult to verify than necessary. Each root cause has one document; code smells are evidence, not separate debts. External/runtime uncertainty belongs in [Architecture Risks](risks.md).

Presence is the state: every document in `technical-debt/` represents debt that currently exists. Do not add `Status` fields or retain resolved debt documents. When a debt is resolved, remove its index entry and delete its document; Git history preserves the former problem and its resolution. Durable constraints or residual runtime uncertainty must first be moved to the appropriate architecture View or risk document.

Current architecture-level technical debt:

- [Native C host textual module boundaries](technical-debt/native-c-host-textual-module-boundaries.md) — Low-medium priority; Linux and Windows host responsibilities are physically separated but still share one C translation unit, so cross-fragment dependencies are not compiler-enforced.
- [Native event-loop validation seam](technical-debt/native-event-loop-validation-seam.md) — Medium priority; native wakeup-to-frame scheduling still lacks a deterministic behavioral test seam independent of real X11, Wayland, or Win32 runtimes.
- [TUI terminal cell width](technical-debt/tui-terminal-cell-width.md) — Low-medium priority; terminal layout still approximates visible width by Unicode scalar count instead of one shared display-cell width authority.
- [Windows AMD Family 10h upstream authority](technical-debt/windows-amd-family10-upstream-authority.md) — Low-medium priority; Family 10h still retains a second temperature-algorithm authority because the pinned official PawnIO transport cannot execute upstream Erratum 319 validation.
- [Windows NDU ABI fixture coverage](technical-debt/windows-ndu-abi-fixture-coverage.md) — Medium-high priority; the undocumented NDU ABI still lacks a representative multi-version raw fixture corpus.
- [Windows NDU end-to-end acceptance](technical-debt/windows-ndu-end-to-end-acceptance.md) — High priority; real process-to-interface NDU attribution still lacks a repeatable end-to-end acceptance harness.
- [Windows ETW disk semantic validation](technical-debt/windows-etw-disk-semantic-validation.md) — High priority; a real-Windows smoke now proves nonzero production attribution and pre-opened-file rundown, but exact successful logical-byte semantics still lack deterministic runtime coverage.
- [Windows console launch authority](technical-debt/windows-console-launch-authority.md) — Medium priority; Windows console preparation still reinterprets raw arguments instead of consuming the CLI/launch authority.
- [Windows native validation infrastructure](technical-debt/windows-native-validation-infrastructure.md) — Medium-high priority; native Windows validation profiles exist, but Linux/WSL development environments have no repository-owned, reproducible host/admin execution path.
