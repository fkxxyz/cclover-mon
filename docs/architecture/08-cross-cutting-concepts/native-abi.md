---
summary: "Defines mechanically verified native ABI contracts for desktop scenes and BPF map records."
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

## NativeScene ABI

The Rust-to-C desktop ABI has one declarative schema authority in `cclover-desktop`. Rust `#[repr(C)]` records/constants and the C `native_scene.h` consumed by Linux and Windows hosts are generated from that schema during the build. Command kinds, flags, field order, pointer types, and callback signatures are not maintained as independent hand-written mirrors.

Font realization is native-host responsibility; layout policy remains in `cclover-ui`. Native hosts measure text with the font they actually realize and expose only text extents through the generated callback contract. `cclover-ui` converts those measurements into shared geometry.

## BPF map ABI

Any BPF map key or value read directly into Rust is a C↔Rust ABI. Rust types use `#[repr(C)]`; the build derives size, alignment, and field offsets from clang's BPF-target record layout and checks the Rust layout at compile time. Schema changes are incomplete until verification covers the complete key/value structure.

Manual visual comparison of duplicate C and Rust declarations is not an acceptable compatibility mechanism.
