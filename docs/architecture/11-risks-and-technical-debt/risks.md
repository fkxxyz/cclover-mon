---
summary: "Records current architecture risks that require evidence during implementation."
viewpoint: assurance
concerns:
  - performance
  - portability
  - maintainability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
---

# Risks and Technical Debt

- Linux layer-shell integration currently depends on `iced_layershell` 0.19.1 and exact `winit-core` / `winit-common` 0.31.0-beta.2 compatibility pins; upgrades require runtime and build validation.
- Native metric parity will vary by operating system; the shared model must preserve common semantics without flattening meaningful platform-specific data.
- Third-party C++ SDKs may impose runtime or packaging costs that must be measured before adoption.
