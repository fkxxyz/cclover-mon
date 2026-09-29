---
summary: "Defines the system-wide strategy for low-overhead collection, cross-platform reuse, and native interoperability."
viewpoint: overview
concerns:
  - architecture-coherence
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

# Solution Strategy

- Produce typed snapshots from native platform backends.
- Keep platform selection at compile time where practical.
- Batch sampling work and reuse buffers on hot paths.
- Keep bounded history in fixed-capacity ring buffers.
- Separate sampling cadence from UI rendering cadence.
- Pass typed data directly inside the native process; JSON serialization is used only at explicit external transport boundaries such as the optional HTTP monitor.
- Derive renderer-neutral dashboard semantics from `MonitorState`, then derive one shared application-specific dashboard tree that owns graphical structure, visual tokens, graph policy, and shared geometry.
- Keep rendering adapters thin: native desktop consumes shared `NativeScene` primitives and uses platform-native hosting/drawing; Web lowers the shared dashboard tree directly to browser-native DOM/CSS/SVG. Neither rebuilds cards independently or introduces a general-purpose GUI framework as UI authority.
- Keep one native sampler as the authority regardless of enabled frontends; desktop, terminal, and HTTP delivery consume the same completed states without creating additional collectors or cadences.
- Prefer event-driven discovery for devices whose lifecycle is exposed by the platform.
