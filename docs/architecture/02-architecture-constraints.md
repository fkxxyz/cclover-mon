---
summary: "Defines hard architecture constraints for runtime efficiency, platform isolation, language boundaries, and build ownership."
viewpoint: overview
concerns:
  - performance
  - portability
  - maintainability
  - security
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
---

# Architecture Constraints

- Rust owns application logic, shared model, sampling, history, presentation semantics, shared dashboard definition, and the top-level build; platform-native code may own desktop windowing/drawing behind a narrow in-process boundary.
- Cargo is the top-level build system.
- Linux and Windows native APIs stay behind platform backends.
- C++ exists only for APIs or SDKs that require C++ and is linked into the same executable.
- Native collection is preferred over periodic subprocess polling.
- Linux privileged collection must request only the authority required by its native mechanism; privileged integration failure must degrade the affected metric to unavailable rather than silently broadening privilege or fabricating zero.
- Production Linux eBPF collection must not require a runtime BCC/Python/clang toolchain or a helper monitoring daemon.
- Shared model and presentation contain no platform handles, platform API types, or renderer toolkit types.
- Native in-process metric flow stays typed; serialization is permitted only at an external transport boundary such as the opt-in HTTP monitor. External Web payloads use an explicit transport projection/allowlist rather than serializing the internal core model directly.
- One renderer-neutral dashboard tree owns graphical UI structure, visual tokens, graph policy, and shared geometry. Native rendering lowers it once into shared `NativeScene`; Web may consume the higher-level tree directly. Renderers must not rebuild monitor cards independently. The terminal frontend owns terminal-specific layout.
- The desktop monitor surface is non-interactive: it must not consume pointer input or block interaction with the desktop or windows beneath it. Native desktop integration must provide pointer pass-through without relying on window-manager-specific rules.
- HTTP monitoring is disabled by default. Non-loopback exposure requires explicit listener configuration; the initial Web surface is read-only and assumes deliberately exposed LANs are trusted.
