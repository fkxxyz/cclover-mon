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

The scene ABI carries invalidation output already derived on the Rust side: static revision, full-redraw fallback, merged damage rectangles, and optional per-command redraw-mask metadata for incremental frames. The mask is frame-local execution metadata aligned with the command stream, not an intrinsic command property; it is derived from the same authoritative primitive bounds used for damage. Native hosts may use those values to drive buffer/cache lifecycle, incremental command culling, renderer-local conformance validation, and platform damage submission, but they do not independently reconstruct primitive visual identity or damage bounds. Validation and execution of an incremental candidate must consume the same final command selection. Missing or malformed redraw-mask metadata must conservatively fall back to the complete selected content class rather than risk underdrawing or under-validating; if a candidate is rejected after an incremental baseline was established, the native host must invalidate that rendered baseline so the next candidate re-establishes pixels and conformance with a full draw.

Font realization is native-host responsibility; layout policy and text-slot geometry remain in `cclover-ui`. The Scene ABI carries whether a text primitive has the must-fit contract, but not font metrics or measured widths. Native renderers may measure the concrete realized string only as a one-way conformance check against the authoritative `Scene::Text` rectangle in final device coordinates. Those measurements stay inside the renderer: they do not cross back through the ABI, move sibling elements, resize slots, or otherwise influence shared layout. A must-fit violation rejects the candidate frame instead of being repaired through renderer-local geometry.

## TUI ABI

The Rust-to-C terminal-frame ABI has one declarative schema authority in `cclover-tui/src/native_abi_spec.rs`. Rust `#[repr(C)]` records and the C `native_tui.h` consumed by `native/tui.c` are generated from that schema during the build; field order and pointer types are not maintained as hand-written mirrors.

## BPF map ABI

Any BPF map key or value read directly into Rust is a C↔Rust ABI. Rust types use `#[repr(C)]`; the build derives size, alignment, and field offsets from clang's BPF-target record layout and checks the Rust layout at compile time. Schema changes are incomplete until verification covers the complete key/value structure.

Manual visual comparison of duplicate C and Rust declarations is not an acceptable compatibility mechanism.
