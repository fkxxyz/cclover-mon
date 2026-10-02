---
summary: "Defines mechanically verified native ABI contracts for desktop scenes, terminal frames, and BPF map records."
viewpoint: static
concerns:
  - architecture-coherence
  - maintainability
  - portability
  - security
activities:
  - orient
  - change
  - assess
facets:
  area:
    - native-bridge
    - platform
---

# Native ABI

Layout-sensitive cross-language contracts have one authority and mechanical verification or generation.

## Scene ABI

The Rust-to-C desktop ABI has one declarative schema authority in `cclover-desktop`. Rust `#[repr(C)]` records/constants and the C `native_scene.h` consumed by Linux and Windows hosts are generated from that schema during the build. Command kinds, flags, field order, pointer types, and callback signatures are not maintained as independent hand-written mirrors.

The scene ABI carries invalidation output already derived on the Rust side: static revision, full-redraw fallback, and merged damage rectangles. Native hosts may use those values to drive buffer/cache lifecycle and platform damage submission, but they do not independently reconstruct primitive visual identity or damage bounds.

Font realization is native-host responsibility; layout policy and text-slot geometry remain in `cclover-ui`. Native font metrics stay inside the renderer and may only position glyphs within the authoritative `Scene::Text` rectangle. They do not cross the ABI or move sibling elements.

## TUI ABI

The Rust-to-C terminal-frame ABI has one declarative schema authority in `cclover-tui/src/native_abi_spec.rs`. Rust `#[repr(C)]` records and the C `native_tui.h` consumed by `native/tui.c` are generated from that schema during the build; field order and pointer types are not maintained as hand-written mirrors.

## BPF map ABI

Any BPF map key or value read directly into Rust is a C↔Rust ABI. Rust types use `#[repr(C)]`; the build derives size, alignment, and field offsets from clang's BPF-target record layout and checks the Rust layout at compile time. Schema changes are incomplete until verification covers the complete key/value structure.

Manual visual comparison of duplicate C and Rust declarations is not an acceptable compatibility mechanism.
