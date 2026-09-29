---
summary: "Tracks metric-centric process projections that will hinder richer process-oriented frontends."
viewpoint: assurance
concerns:
  - maintainability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
---

# Process-Domain Projection

## Problem

Process data is currently projected primarily by metric (`ProcessCpuUsage`, `ProcessMemoryUsage`, process disk I/O, process network I/O) rather than through a coherent process-oriented view. Top CPU/memory projections also discard stable process identity because the current desktop only needs display rows.

## Evidence

The core has a stable `ProcessInstanceId`, but `ProcessCpuUsage` and `ProcessMemoryUsage` contain only name plus value, while I/O projections retain process identity independently. Presentation consumes separate Top-N lists rather than a joined process entity model.

## Maintenance impact

The current metric-card TUI does not require a unified process table, but an htop-like process view or richer process drill-down will need to join CPU, memory, disk, and network data by process. The current projections can force ad-hoc joins, duplicated lookup logic, or a disruptive model redesign when that frontend arrives.

## Governing constraint

Do not introduce a full process domain model before product requirements need it, but retain enough stable identity and ownership semantics that richer process views can be built without reconstructing identity from display data.

## Resolution direction

Revisit process projection when a process-oriented frontend or feature is implemented. Prefer a shared process-centric projection or stable keyed views over frontend-specific joins.

## Exit criteria

A process-oriented frontend can consume stable joined process metrics without inventing its own identity/join model or changing unrelated collectors.
