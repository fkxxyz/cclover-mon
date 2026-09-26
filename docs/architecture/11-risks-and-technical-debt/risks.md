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
- Probe availability is partly inferred from diagnostic text in the Linux backend. Diagnostic wording must not carry program semantics; collector status should become typed before probe reporting grows further.
- Linux collection is concentrated in one large backend module that owns CPU, memory, process, network, disk, temperature, retry/cache state, and probe formatting. Split collectors by metric responsibility while keeping `Backend` as the batch composition point so new metrics do not expand one central module and match tree.
- UI sizing has two authorities: the actual Iced layout and the hand-maintained `panel_height` arithmetic. Layout-affecting dimensions should have one shared authority so visual changes cannot silently desynchronize window sizing. The current `ui -> app::Message` dependency should also be removed when this boundary is cleaned up.
- Platform desktop integration ownership does not yet match the declared architecture: Linux layer-shell and Windows window bootstrap remain in `main.rs` rather than the platform boundary. Move this when platform-specific window behavior expands, especially as the Windows backend becomes real.
- Native parsing and filesystem IO are still coupled in several Linux collectors. Extract pure parsers for `/proc` and `/sys` formats and cover them with fixture-style tests so collector changes can be validated without depending on the developer machine's live filesystem state.
