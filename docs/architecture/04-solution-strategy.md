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
- Derive renderer-neutral dashboard presentation from `MonitorState`, then let each frontend own its rendering and layout policy.
- Reuse one Iced panel implementation across Linux/Windows native desktop and browser/WASM delivery; a future terminal frontend reuses core and presentation semantics rather than Iced widgets or pixel layout.
- Keep one native sampler as the authority when HTTP monitoring is enabled; Web clients consume bounded complete-state updates and never start collectors.
- Prefer event-driven discovery for devices whose lifecycle is exposed by the platform.
