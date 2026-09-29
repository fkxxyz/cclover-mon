---
summary: "Indexes active architecture-level technical debt by root cause and priority."
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

## P1

| Debt | Status | Why it matters |
| --- | --- | --- |
| [Architecture enforcement](technical-debt/architecture-enforcement.md) | Active | Important dependency and validation rules rely too heavily on developer memory and manual execution. |

## P2

| Debt | Status | Why it matters |
| --- | --- | --- |
| [GPU capability contract](technical-debt/gpu-capability-contract.md) | Active | GPU ownership and missing-field semantics still rely on cross-collector convention and compressed optional state, raising vendor/backend extension risk. |
| [Native text metrics contract](technical-debt/native-text-metrics-contract.md) | Active | Native absolute text geometry still assumes preferred-font metrics before Cairo/GDI realize or substitute the actual face. |

## P3

| Debt | Status | Why it matters |
| --- | --- | --- |
| [Process-domain projection](technical-debt/process-domain-projection.md) | Active | Process data is projected per metric, which will complicate richer process-oriented frontends. |
| [Pinned Iced WebGL workaround](technical-debt/pinned-iced-webgl-workaround.md) | Active | Browser Canvas correctness currently depends on a maintained fork of `iced_widget`. |
