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

- Rust owns application logic, shared model, sampling, history, UI behavior, and the top-level build.
- Cargo is the top-level build system.
- Linux and Windows native APIs stay behind platform backends.
- C++ exists only for APIs or SDKs that require C++ and is linked into the same executable.
- Native collection is preferred over periodic subprocess polling.
- Shared UI and model contain no platform handles or platform API types.
