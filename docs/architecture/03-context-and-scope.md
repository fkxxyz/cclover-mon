---
summary: "Defines the cclover-mon system boundary and its relationships with the operating system and optional native libraries."
viewpoint: overview
concerns:
  - architecture-coherence
  - portability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
---

# Context and Scope

```text
Operating system / native libraries
             ↓
      Platform backend
             ↓
       Shared snapshot
        ↙          ↘
 native Iced     HTTP/SSE
     UI              ↓
                 browser/WASM
                 shared Iced UI
```

The native application reads local machine state. Platform backends own OS-specific access. The same Iced panel implementation presents state locally and, when explicitly enabled, in remote browsers through an HTTP/SSE boundary. Optional C++ bridges adapt C++-only libraries to a narrow C ABI consumed by Rust.
