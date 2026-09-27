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
| [Metric extension coupling](technical-debt/metric-extension-coupling.md) | Active | Adding a metric propagates through multiple central switchboards and authorities. |
| [Collector outcome semantics](technical-debt/collector-outcome-semantics.md) | Active | Empty, unavailable, partial failure, and diagnostics are not represented by one typed contract. |
| [Desktop hosting boundary](technical-debt/desktop-hosting-boundary.md) | Active | Linux display-protocol policy leaks into application composition and has multiple authorities. |
| [Architecture enforcement](technical-debt/architecture-enforcement.md) | Active | Important dependency and validation rules rely too heavily on developer memory and manual execution. |

## P2

| Debt | Status | Why it matters |
| --- | --- | --- |
| [Dynamic-series identity](technical-debt/dynamic-series-identity.md) | Active | Network and disk history still use mutable names as persistent identity. |
| [Native collector test seams](technical-debt/native-collector-test-seams.md) | Active | Discovery and filesystem/native IO remain expensive to validate without the live host. |
| [BPF ABI schema duplication](technical-debt/bpf-abi-schema-duplication.md) | Active | C and Rust independently define map key/value layouts that must stay synchronized. |
| [Native FFI safety boundary](technical-debt/native-ffi-safety-boundary.md) | Active | Raw FFI and unsafe lifecycle code are mixed with metric semantics. |
| [Frontend layout authority](technical-debt/frontend-layout-authority.md) | Active | Rendered widget geometry and manually calculated panel height must remain synchronized. |
| [eBPF build coupling](technical-debt/ebpf-build-coupling.md) | Active | Runtime-optional attribution remains a mandatory Linux build dependency. |

## P3

| Debt | Status | Why it matters |
| --- | --- | --- |
| [Process-domain projection](technical-debt/process-domain-projection.md) | Active | Process data is projected per metric, which will complicate richer process-oriented frontends. |
| [Low-risk cleanup queue](technical-debt/low-risk-cleanups.md) | Active | Small local cleanups are intentionally deferred but should not be promoted into separate architecture debts. |
