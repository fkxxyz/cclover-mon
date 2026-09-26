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
2. Keep one shared data model and one shared UI across platforms.
3. Add new metrics through native system interfaces without coupling them to presentation.
4. Keep high-frequency UI iteration independent from recompiling system logic where the UI toolkit permits reloadable resources.

## Scope

- Native collection of CPU, memory, process, disk, network, sensor, and future system metrics.
- Linux first; Windows is a first-class platform backend.
- One application process and one final executable artifact per target platform.
