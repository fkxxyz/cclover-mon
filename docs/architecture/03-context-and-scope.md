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
       presentation
             ↓
       shared dashboard tree
         ↙          ↘
 NativeScene      HTTP/SSE
     ↓                ↓
 native renderer  Web renderer
```

The native application reads local machine state. Platform backends own OS-specific access. Graphical frontends share one renderer-neutral dashboard definition but realize it through platform-native desktop rendering or a browser renderer. Remote browsers receive state only through the explicit HTTP/SSE boundary. Optional C++ bridges adapt native-only implementation code through narrow in-process ABIs.
