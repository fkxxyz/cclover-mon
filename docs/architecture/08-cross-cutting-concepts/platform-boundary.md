---
summary: "Indexes platform isolation, native API, language, identity, and ABI boundaries."
viewpoint: static
concerns:
  - architecture-coherence
  - performance
  - portability
  - maintainability
  - security
activities:
  - orient
  - change
  - assess
facets:
  area:
    - platform
    - native-bridge
---

# Platform Boundary

Platform backends implement core-owned collection contracts and translate native state into core-owned platform-neutral types. `core` never imports a platform backend. Native desktop startup enters the shared `cclover-desktop` host contract; platform hosts own native realization.

Cross-cutting platform rules are split by responsibility:

- [Native Language Boundary](native-language-boundary.md) — safe Rust, C/C++ selection, and FFI ownership.
- [Stable Identity](stable-identity.md) — native locators and cross-source canonical identity.
- [Linux Platform Boundary](linux-platform-boundary.md) — Linux collectors, GPU/hwmon reconciliation, eBPF, desktop integration, and diagnostics seams.
- [Windows Platform Boundary](windows-platform-boundary.md) — Windows collector sources, PawnIO/GPU runtime ownership, identity, and desktop integration.
- [Native ABI](native-abi.md) — Rust/C layout authority, generated `NativeScene` contracts, and BPF map ABI verification.

Language-selection rationale is owned by [ADR 001](../09-architecture-decisions/001-rust-core-native-boundaries.md). PawnIO provisioning and distribution are owned by [ADR 009](../09-architecture-decisions/009-windows-pawnio-provisioning.md). Runtime eBPF attribution details are owned by the [Linux eBPF I/O Attribution](../06-runtime-view/linux-ebpf-io-attribution.md) View.
