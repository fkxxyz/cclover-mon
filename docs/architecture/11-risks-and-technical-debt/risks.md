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

- The UI toolkit is not yet selected; it must satisfy cross-platform support, low runtime overhead, and fast visual iteration.
- Native metric parity will vary by operating system; the shared model must preserve common semantics without flattening meaningful platform-specific data.
- Third-party C++ SDKs may impose runtime or packaging costs that must be measured before adoption.
