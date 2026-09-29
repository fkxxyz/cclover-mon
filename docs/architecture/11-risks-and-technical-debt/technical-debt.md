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

No current architecture-level technical debt is recorded.
