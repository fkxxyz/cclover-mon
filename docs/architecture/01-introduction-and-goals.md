---
summary: "Defines cclover-mon purpose, scope, architecture drivers, and quality priorities."
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

# Introduction and Goals

`cclover-mon` is a low-overhead native system monitor for Linux and Windows.

## Drivers

1. Minimize steady-state CPU, memory, wakeups, allocation, and data movement.
2. Keep one shared metric model, renderer-neutral presentation semantics, and one shared graphical dashboard definition across native desktop and Web renderers.
3. Add new metrics through native system interfaces without coupling them to presentation.

## Scope

- Native collection of CPU, memory, process, disk, network, sensor, and future system metrics.
- Linux first; Windows is a first-class platform backend.
- One shared graphical dashboard definition consumed by platform-native desktop renderers (through `NativeScene`) and optional Web delivery; the terminal frontend reuses core and presentation semantics with terminal-specific layout.
- One native application process and one final executable artifact per target platform; optional Web assets are embedded and served by that process.
