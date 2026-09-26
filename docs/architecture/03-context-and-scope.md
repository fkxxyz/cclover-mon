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
             ↓
         cclover-mon UI
```

The application reads local machine state and presents it locally. Platform backends own OS-specific access. Optional C++ bridges adapt C++-only libraries to a narrow C ABI consumed by Rust.
