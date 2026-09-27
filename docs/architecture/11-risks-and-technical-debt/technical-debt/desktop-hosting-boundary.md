---
summary: "Tracks incomplete ownership of Linux desktop hosting and display-protocol policy."
viewpoint: assurance
concerns:
  - maintainability
  - portability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
---

# Desktop Hosting Boundary

## Status

Active — P1.

## Problem

Desktop hosting is declared a platform responsibility, but Linux display-protocol policy is still split across `main`, `app`, and `platform::linux::desktop`. The platform boundary therefore does not yet provide one complete authority for monitor-surface hosting.

## Evidence

`main.rs` directly configures `iced_layershell`, Wayland anchor/layer/exclusive-zone/input policy, and display-server selection. `app.rs` knows X11-specific resize behavior and layer-shell size messages. `desktop.rs` combines display detection, tray integration, X11/EWMH policy, input shape, and placement. Wayland and X11 placement margins are also represented independently.

## Maintenance impact

Changes to hosting policy can require coordinated edits across application and platform layers. Adding Windows desktop integration or changing Linux hosting behavior risks further protocol leakage and duplicated policy.

## Governing constraint

Application and shared UI code should express platform-neutral lifecycle and desired surface size. Native display protocols, placement policy, shell hints, tray protocols, and protocol-specific resize mechanics belong behind platform desktop integration.

## Resolution direction

Converge monitor hosting on a platform-owned interface while keeping application lifecycle semantics outside native callbacks. Split independent desktop responsibilities internally where useful; do not hide product-level lifecycle decisions inside platform code.

## Exit criteria

`main` and shared `app` code no longer contain Linux display-protocol concepts or duplicated placement policy, and each supported desktop platform can host the same frontend through a platform-owned boundary.
