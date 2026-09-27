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
- Pass typed data directly inside the process; no internal JSON or frontend/backend IPC.
- Derive renderer-neutral dashboard presentation from `MonitorState`, then let each frontend own its rendering and layout policy.
- Reuse the Iced desktop frontend across Linux and Windows; a future terminal frontend reuses core and presentation semantics rather than Iced widgets or pixel layout.
- Prefer event-driven discovery for devices whose lifecycle is exposed by the platform.
