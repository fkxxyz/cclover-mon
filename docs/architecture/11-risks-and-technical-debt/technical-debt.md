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
| [Collector outcome semantics](technical-debt/collector-outcome-semantics.md) | Active | Typed availability/degradation is lost after raw collection, making downstream state and history conflate distinct observations. |
| [Metric extension coupling](technical-debt/metric-extension-coupling.md) | Active | Adding a metric propagates through multiple central switchboards and authorities. |
| [Desktop hosting boundary](technical-debt/desktop-hosting-boundary.md) | Active | Linux display-protocol policy leaks into application composition and has multiple authorities. |
| [Architecture enforcement](technical-debt/architecture-enforcement.md) | Active | Important dependency and validation rules rely too heavily on developer memory and manual execution. |

## P2

| Debt | Status | Why it matters |
| --- | --- | --- |
| [Dynamic-series identity](technical-debt/dynamic-series-identity.md) | Active | Process I/O attribution still uses mutable device/interface names as cross-sample identity instead of shared core identities. |
| [Native collector test seams](technical-debt/native-collector-test-seams.md) | Active | Discovery and filesystem/native IO remain expensive to validate without the live host. |
| [Optional capability build coupling](technical-debt/optional-capability-build-coupling.md) | Active | Runtime-optional eBPF and Web capabilities still impose mandatory toolchain and artifact costs on normal native builds. |

## P3

| Debt | Status | Why it matters |
| --- | --- | --- |
| [Process-domain projection](technical-debt/process-domain-projection.md) | Active | Process data is projected per metric, which will complicate richer process-oriented frontends. |
| [Pinned Iced WebGL workaround](technical-debt/pinned-iced-webgl-workaround.md) | Active | Browser Canvas correctness currently depends on a maintained fork of `iced_widget`. |
