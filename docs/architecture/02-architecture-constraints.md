---
summary: "Defines hard architecture constraints for runtime efficiency, platform isolation, language boundaries, and build ownership."
viewpoint: overview
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

# Architecture Constraints

- Rust owns application logic, shared model, sampling, history, presentation semantics, frontend behavior, and the top-level build.
- Cargo is the top-level build system.
- Linux and Windows native APIs stay behind platform backends.
- C++ exists only for APIs or SDKs that require C++ and is linked into the same executable.
- Native collection is preferred over periodic subprocess polling.
- Shared model and presentation contain no platform handles, platform API types, or renderer toolkit types.
- Frontend-specific layout stays in the frontend that renders it; desktop pixel layout is not a shared contract for a future terminal frontend.
- The desktop monitor surface is non-interactive: it must not consume pointer input or block interaction with the desktop or windows beneath it. Native desktop integration must provide pointer pass-through without relying on window-manager-specific rules.
